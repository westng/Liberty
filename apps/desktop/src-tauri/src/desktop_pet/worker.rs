#[cfg(target_os = "macos")]
use super::macos_pet_renderer;
#[cfg(windows)]
use super::windows_pet_renderer;
use crate::local_db::{self, LocalResult};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Manager, Window};

use super::{
    append_diagnostic, behavior, DesktopPetInstance, DesktopPetState, PetAction, PetBubble,
    PetBubbleTheme, PetGrowthFloat, PetInstanceId, PetVisualState, PetWorker, PetWorkerContext,
    ANIMATION_FRAME_MS, BUBBLE_VISIBLE_MS, PET_RENDER_TICK_MS, PET_WINDOW_HEIGHT, PET_WINDOW_WIDTH,
    STATE_REFRESH_MS,
};

pub(crate) fn create_pet_window(
    app: &AppHandle,
    visual_state: &PetVisualState,
    instance_id: PetInstanceId,
    interaction_signal: &Arc<AtomicU64>,
    persist_position: bool,
) -> LocalResult<Window> {
    let position =
        behavior::resolve_pet_position(app, visual_state, instance_id, persist_position)?;
    let window = tauri::WindowBuilder::new(app, instance_id.label())
        .title("Liberty Pet")
        .inner_size(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT)
        .min_inner_size(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT)
        .max_inner_size(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT)
        .position(position.x, position.y)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(visual_state.always_on_top)
        .skip_taskbar(true)
        .shadow(false)
        .focused(false)
        .focusable(false)
        .visible(true)
        .build()
        .map_err(|err| err.to_string())?;
    match window.set_ignore_cursor_events(false) {
        Ok(()) => append_diagnostic(
            app,
            format!(
                "window ignore cursor disabled instance={} label={}",
                instance_id.log_name(),
                instance_id.label()
            ),
        ),
        Err(error) => append_diagnostic(
            app,
            format!(
                "window ignore cursor disable failed instance={} error={error}",
                instance_id.log_name()
            ),
        ),
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = interaction_signal;

    #[cfg(windows)]
    {
        let window_clone = window.clone();
        let signal = interaction_signal.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        window
            .run_on_main_thread(move || {
                let result = windows_pet_renderer::prepare_window(&window_clone, &signal);
                let _ = sender.send(result);
            })
            .map_err(|err| err.to_string())?;
        receiver
            .recv()
            .map_err(|_| "Windows 桌宠主线程初始化结果丢失。".to_string())??;
    }
    #[cfg(target_os = "macos")]
    {
        let window_clone = window.clone();
        let signal = interaction_signal.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        window
            .run_on_main_thread(move || {
                let result = macos_pet_renderer::prepare_window(&window_clone, signal);
                let _ = sender.send(result);
            })
            .map_err(|err| err.to_string())?;
        receiver
            .recv()
            .map_err(|_| "macOS 桌宠主线程初始化结果丢失。".to_string())??;
    }

    eprintln!(
        "[desktop-pet] native window created instance={} at x={:.0}, y={:.0}, action={}",
        instance_id.log_name(),
        position.x,
        position.y,
        visual_state.action.as_str()
    );
    if let Ok(physical_position) = window.outer_position() {
        eprintln!(
            "[desktop-pet] native window physical position x={}, y={}",
            physical_position.x, physical_position.y
        );
        append_diagnostic(
            app,
            format!(
                "window created instance={} label={} physical=({}, {}) logical=({:.0}, {:.0}) action={}",
                instance_id.log_name(),
                instance_id.label(),
                physical_position.x,
                physical_position.y,
                position.x,
                position.y,
                visual_state.action.as_str()
            ),
        );
    }

    Ok(window)
}

pub(crate) fn configure_pet_window(
    app: &AppHandle,
    window: &Window,
    visual_state: &PetVisualState,
    instance_id: PetInstanceId,
    persist_position: bool,
) -> LocalResult<()> {
    window
        .set_always_on_top(visual_state.always_on_top)
        .map_err(|err| err.to_string())?;
    if persist_position {
        let position =
            behavior::resolve_pet_position(app, visual_state, instance_id, persist_position)?;
        window
            .set_position(position)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn refresh_pet_window_state(window: &Window, visual_state: &PetVisualState) -> LocalResult<()> {
    window
        .set_always_on_top(visual_state.always_on_top)
        .map_err(|err| err.to_string())
}

pub(crate) fn ensure_worker(
    app: &AppHandle,
    window: &Window,
    action: PetAction,
    instance_id: PetInstanceId,
) -> LocalResult<PetWorker> {
    let state = app.state::<Mutex<DesktopPetState>>();
    let interaction_signal = {
        let mut guard = state
            .lock()
            .map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
        let instance = get_or_create_instance(&mut guard, instance_id);
        if let Some(worker) = instance.worker.clone() {
            return Ok(worker);
        }
        instance.interaction_signal.clone()
    };

    let action_state = Arc::new(Mutex::new(action));
    let bubble_state = Arc::new(Mutex::new(None));
    let growth_float_state = Arc::new(Mutex::new(None));
    let stop_signal = Arc::new(AtomicBool::new(false));
    let worker = PetWorker {
        action: action_state.clone(),
    };
    let app_for_thread = app.clone();
    let window_for_thread = window.clone();
    let stop_for_thread = stop_signal.clone();
    let frames = behavior::load_pet_animation_frames(app)?;

    std::thread::Builder::new()
        .name(format!("liberty-desktop-pet-{}", instance_id.log_name()))
        .spawn(move || {
            run_pet_worker(PetWorkerContext {
                app: app_for_thread,
                window: window_for_thread,
                instance_id,
                action_state,
                bubble_state,
                growth_float_state,
                stop_signal: stop_for_thread,
                interaction_signal,
                frames,
            });
        })
        .map_err(|err| err.to_string())?;

    let mut guard = state
        .lock()
        .map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
    let instance = get_or_create_instance(&mut guard, instance_id);
    instance.stop_signal = Some(stop_signal);
    Ok(worker)
}

impl PetWorker {
    pub(crate) fn set_action(&self, action: PetAction) {
        if let Ok(mut guard) = self.action.lock() {
            *guard = action;
        }
    }
}

fn run_pet_worker(context: PetWorkerContext) {
    let PetWorkerContext {
        app,
        window,
        instance_id,
        action_state,
        bubble_state,
        growth_float_state,
        stop_signal,
        interaction_signal,
        frames,
        ..
    } = context;
    let mut frame_index = 0usize;
    let mut last_action = PetAction::Slack;
    let mut last_refresh = SystemTime::now();
    let mut last_proactive_bubble = SystemTime::now();
    let mut bubble_theme = resolve_bubble_theme(&app, &window);
    #[cfg(any(windows, target_os = "macos"))]
    let mut first_frame_logged = false;
    let mut handled_interactions = interaction_signal.load(Ordering::Relaxed);
    let mut last_speech_event_id = String::new();
    append_diagnostic(
        &app,
        format!("worker started instance={}", instance_id.log_name()),
    );

    while !stop_signal.load(Ordering::Relaxed) {
        let pending_interactions = interaction_signal.load(Ordering::Relaxed);
        if pending_interactions != handled_interactions {
            handled_interactions = pending_interactions;
            behavior::handle_desktop_pet_interaction(&app, &action_state, &bubble_state);
        }

        if last_refresh.elapsed().unwrap_or_default() >= Duration::from_millis(STATE_REFRESH_MS) {
            if let Ok(settings) = local_db::get_pet_settings(&app) {
                if !settings.desktop_enabled {
                    let app_clone = app.clone();
                    app.run_on_main_thread(move || {
                        if let Err(error) = super::hide_desktop_pet_inner(&app_clone) {
                            eprintln!("[desktop-pet] failed to hide disabled pet: {error}");
                        }
                    })
                    .ok();
                    break;
                }

                if let Ok(visual_state) = behavior::resolve_visual_state(&app, &settings) {
                    if let Ok(mut guard) = action_state.lock() {
                        *guard = visual_state.action;
                    }
                    let window_clone = window.clone();
                    window
                        .run_on_main_thread(move || {
                            if let Err(error) =
                                refresh_pet_window_state(&window_clone, &visual_state)
                            {
                                eprintln!("[desktop-pet] failed to configure window: {error}");
                            }
                        })
                        .ok();
                }

                if let Ok(Some(event)) =
                    local_db::list_pet_event_ledger(&app, 1).map(|events| events.into_iter().next())
                {
                    if event.id != last_speech_event_id {
                        last_speech_event_id = event.id.clone();
                        if let Some(line) = behavior::speech_line_from_event(&event) {
                            if let Ok(mut guard) = bubble_state.lock() {
                                *guard = Some(PetBubble {
                                    text: line.clone(),
                                    expires_at: SystemTime::now()
                                        + Duration::from_millis(BUBBLE_VISIBLE_MS),
                                });
                            }
                            eprintln!("[desktop-pet] event bubble text={line}");
                        }
                        if event.event_type == "store_food" && event.event_value > 0 {
                            if let Ok(mut guard) = growth_float_state.lock() {
                                *guard = Some(PetGrowthFloat {
                                    value: event.event_value,
                                    started_at: SystemTime::now(),
                                    expires_at: SystemTime::now() + Duration::from_millis(3_000),
                                });
                            }
                        }
                    }
                }

                if behavior::should_show_proactive_bubble(&settings, last_proactive_bubble) {
                    let line = behavior::select_proactive_dialogue(&app);
                    if let Ok(mut guard) = bubble_state.lock() {
                        *guard = Some(PetBubble {
                            text: line.clone(),
                            expires_at: SystemTime::now()
                                + Duration::from_millis(BUBBLE_VISIBLE_MS),
                        });
                    }
                    last_proactive_bubble = SystemTime::now();
                    eprintln!("[desktop-pet] proactive bubble text={line}");
                }
            }
            bubble_theme = resolve_bubble_theme(&app, &window);
            last_refresh = SystemTime::now();
        }

        let action = action_state
            .lock()
            .map(|guard| *guard)
            .unwrap_or(PetAction::Slack);
        let bubble_text = behavior::current_bubble_text(&bubble_state);
        let growth_float = behavior::current_growth_float(&growth_float_state);
        if action != last_action {
            frame_index = 0;
            last_action = action;
        }

        let frame_path = match frames.frame_path(action, frame_index) {
            Ok(path) => path.clone(),
            Err(error) => {
                eprintln!("[desktop-pet] failed to resolve sprite frame: {error}");
                std::thread::sleep(Duration::from_millis(ANIMATION_FRAME_MS));
                continue;
            }
        };

        #[cfg(windows)]
        {
            let window_clone = window.clone();
            let frame_for_log = frame_path.clone();
            let should_log_frame = !first_frame_logged;
            let growth_float_for_render = growth_float.clone();
            window
                .run_on_main_thread(move || {
                    if let Err(error) = windows_pet_renderer::paint_window(
                        &window_clone,
                        &frame_path,
                        bubble_text.as_deref(),
                        growth_float_for_render.as_ref(),
                        bubble_theme,
                    ) {
                        eprintln!("[desktop-pet] failed to paint window: {error}");
                    } else if should_log_frame {
                        eprintln!(
                            "[desktop-pet] painted first frame {}",
                            frame_for_log.display()
                        );
                    }
                })
                .ok();
            first_frame_logged = true;
        }

        #[cfg(target_os = "macos")]
        {
            let window_clone = window.clone();
            let frame_for_log = frame_path.clone();
            let should_log_frame = !first_frame_logged;
            let growth_float_for_render = growth_float.clone();
            window
                .run_on_main_thread(move || {
                    if let Err(error) = macos_pet_renderer::paint_window(
                        &window_clone,
                        &frame_path,
                        bubble_text.as_deref(),
                        growth_float_for_render.as_ref(),
                        bubble_theme,
                    ) {
                        eprintln!("[desktop-pet] failed to paint window: {error}");
                    } else if should_log_frame {
                        eprintln!(
                            "[desktop-pet] painted first frame {}",
                            frame_for_log.display()
                        );
                    }
                })
                .ok();
            first_frame_logged = true;
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (
                &window,
                &frame_path,
                bubble_text.as_deref(),
                bubble_theme.is_dark(),
                growth_float
                    .as_ref()
                    .map(|value| (value.value, value.started_at)),
            );
        }

        if growth_float.is_none() {
            frame_index = frame_index.saturating_add(1);
            std::thread::sleep(Duration::from_millis(ANIMATION_FRAME_MS));
        } else {
            std::thread::sleep(Duration::from_millis(PET_RENDER_TICK_MS));
            if frame_index == 0 {
                frame_index = frame_index.saturating_add(1);
            }
        }
    }
}

fn resolve_bubble_theme(app: &AppHandle, window: &Window) -> PetBubbleTheme {
    match local_db::get_settings(app).map(|settings| settings.theme_mode) {
        Ok(theme_mode) if theme_mode == "light" => PetBubbleTheme::Light,
        Ok(theme_mode) if theme_mode == "dark" => PetBubbleTheme::Dark,
        _ => match window.theme() {
            Ok(tauri::Theme::Dark) => PetBubbleTheme::Dark,
            _ => PetBubbleTheme::Light,
        },
    }
}

pub(crate) fn get_or_create_instance(
    state: &mut DesktopPetState,
    instance_id: PetInstanceId,
) -> &mut DesktopPetInstance {
    if let Some(index) = state
        .instances
        .iter()
        .position(|instance| instance.id == instance_id)
    {
        return &mut state.instances[index];
    }

    state.instances.push(DesktopPetInstance {
        id: instance_id,
        window: None,
        worker: None,
        stop_signal: None,
        interaction_signal: Arc::new(AtomicU64::new(0)),
    });
    state
        .instances
        .last_mut()
        .expect("desktop pet instance was just inserted")
}
