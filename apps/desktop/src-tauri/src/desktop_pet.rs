use crate::local_db::{self, LocalResult, PetEventLedgerEntry, PetSettings};
use chrono::Utc;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, LogicalPosition, Manager, Runtime, Window, WindowEvent};

const PET_WINDOW_LABEL: &str = "desktop-pet";
const EXTRA_PET_WINDOW_LABEL_PREFIX: &str = "desktop-pet-extra";
const PET_WINDOW_WIDTH: f64 = 320.0;
const PET_WINDOW_HEIGHT: f64 = 220.0;
#[cfg(any(windows, target_os = "macos"))]
const PET_SPRITE_WIDTH: u32 = 148;
#[cfg(any(windows, target_os = "macos"))]
const PET_SPRITE_HEIGHT: u32 = 148;
const ANIMATION_FRAME_MS: u64 = 1000;
const STATE_REFRESH_MS: u64 = 5000;
const RECENT_EVENT_ACTION_HOLD_MS: u64 = 45_000;
const NEEDY_AFTER_MS: u64 = 30_000;
const DAILY_ACTION_BUCKET_MS: u64 = 120_000;
const BUBBLE_VISIBLE_MS: u64 = 7_000;
const EXTRA_PET_OFFSET_X: f64 = 168.0;
const EXTRA_PET_OFFSET_Y: f64 = 92.0;

struct DesktopPetState {
    instances: Vec<DesktopPetInstance>,
    next_extra_id: u64,
}

struct DesktopPetInstance {
    id: PetInstanceId,
    window: Option<Window<tauri::Wry>>,
    worker: Option<PetWorker>,
    stop_signal: Option<Arc<AtomicBool>>,
    interaction_signal: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PetInstanceId {
    Primary,
    Extra(u64),
}

impl PetInstanceId {
    fn label(self) -> String {
        match self {
            Self::Primary => PET_WINDOW_LABEL.to_string(),
            Self::Extra(id) => format!("{EXTRA_PET_WINDOW_LABEL_PREFIX}-{id}"),
        }
    }

    fn log_name(self) -> String {
        match self {
            Self::Primary => "primary".to_string(),
            Self::Extra(id) => format!("extra-{id}"),
        }
    }
}

#[derive(Clone)]
struct PetWorker {
    action: Arc<Mutex<PetAction>>,
}

#[derive(Debug, Clone)]
struct PetBubble {
    text: String,
    expires_at: SystemTime,
}

struct PetAnimationFrames {
    crush: Vec<PathBuf>,
    defecate: Vec<PathBuf>,
    drive: Vec<PathBuf>,
    eat: Vec<PathBuf>,
    pants: Vec<PathBuf>,
    read: Vec<PathBuf>,
    rope: Vec<PathBuf>,
    run: Vec<PathBuf>,
    slack: Vec<PathBuf>,
    sleep: Vec<PathBuf>,
    snow: Vec<PathBuf>,
    toy: Vec<PathBuf>,
    work: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PetAction {
    Crush,
    Defecate,
    Drive,
    Eat,
    Pants,
    Read,
    Rope,
    Run,
    Slack,
    Sleep,
    Snow,
    Toy,
    Work,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PetBubbleTheme {
    Light,
    Dark,
}

impl PetBubbleTheme {
    fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

impl PetAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Crush => "crush",
            Self::Defecate => "defecate",
            Self::Drive => "drive",
            Self::Eat => "eat",
            Self::Pants => "pants",
            Self::Read => "read",
            Self::Rope => "rope",
            Self::Run => "run",
            Self::Slack => "slack",
            Self::Sleep => "sleep",
            Self::Snow => "snow",
            Self::Toy => "toy",
            Self::Work => "work",
        }
    }
}

#[derive(Debug, Clone)]
struct PetVisualState {
    action: PetAction,
    always_on_top: bool,
    last_window_x: Option<f64>,
    last_window_y: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPetStatus {
    pub visible: bool,
    pub instance_count: usize,
}

pub fn manage_desktop_pet_state<R: Runtime>(app: &tauri::AppHandle<R>) {
    app.manage(Mutex::new(DesktopPetState {
        instances: Vec::new(),
        next_extra_id: 1,
    }));
}

pub fn sync_desktop_pet_on_startup(app: &AppHandle) {
    match show_desktop_pet_inner(app) {
        Ok(visible) => {
            eprintln!("[desktop-pet] startup sync completed visible={visible}");
        }
        Err(error) => {
            eprintln!("[desktop-pet] startup sync failed: {error}");
        }
    }
}

#[tauri::command]
pub async fn show_desktop_pet(app: AppHandle, source: Option<String>) -> LocalResult<bool> {
    eprintln!(
        "[desktop-pet] show command received source={}",
        source.as_deref().unwrap_or("unknown")
    );
    show_desktop_pet_inner(&app)
}

fn show_desktop_pet_inner(app: &AppHandle) -> LocalResult<bool> {
    let settings = local_db::get_pet_settings(&app)?;
    eprintln!(
        "[desktop-pet] settings desktop_enabled={}, always_on_top={}",
        settings.desktop_enabled, settings.always_on_top
    );
    if !settings.desktop_enabled {
        return hide_desktop_pet_inner(app);
    }

    show_desktop_pet_instance(app, PetInstanceId::Primary, true)?;
    Ok(true)
}

fn show_desktop_pet_instance(app: &AppHandle, instance_id: PetInstanceId, persist_position: bool) -> LocalResult<()> {
    let settings = local_db::get_pet_settings(&app)?;
    if !settings.desktop_enabled {
        hide_desktop_pet_inner(app)?;
        return Ok(());
    }
    eprintln!(
        "[desktop-pet] show instance begin instance={} persist_position={persist_position}",
        instance_id.log_name()
    );

    let visual_state = resolve_visual_state(&app, &settings)?;
    let (existing_window, interaction_signal) = {
        let state = app.state::<Mutex<DesktopPetState>>();
        let mut guard = state.lock().map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
        let instance = get_or_create_instance(&mut guard, instance_id);
        (instance.window.take(), instance.interaction_signal.clone())
    };

    let window = if let Some(window) = existing_window {
        eprintln!("[desktop-pet] reuse existing window instance={}", instance_id.log_name());
        configure_pet_window(app, &window, &visual_state, instance_id, persist_position)?;
        window.show().map_err(|err| err.to_string())?;
        window
    } else {
        eprintln!("[desktop-pet] create new window instance={}", instance_id.log_name());
        let window = create_pet_window(&app, &visual_state, instance_id, &interaction_signal, persist_position)?;
        if persist_position {
            attach_window_position_persistence(&app, &window);
        }
        window
    };

    eprintln!("[desktop-pet] ensure worker instance={}", instance_id.log_name());
    let worker = ensure_worker(&app, &window, visual_state.action, instance_id)?;
    worker.set_action(visual_state.action);

    let state = app.state::<Mutex<DesktopPetState>>();
    let mut guard = state.lock().map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
    let instance = get_or_create_instance(&mut guard, instance_id);
    instance.window = Some(window);
    instance.worker = Some(worker);
    eprintln!("[desktop-pet] show instance completed instance={}", instance_id.log_name());
    Ok(())
}

#[tauri::command]
pub async fn open_extra_desktop_pet(app: AppHandle) -> LocalResult<DesktopPetStatus> {
    eprintln!("[desktop-pet] open extra command received");
    let settings = local_db::get_pet_settings(&app)?;
    if !settings.desktop_enabled {
        let mut next_settings = settings;
        next_settings.desktop_enabled = true;
        next_settings.updated_at = Utc::now().to_rfc3339();
        local_db::save_pet_settings(&app, &next_settings)?;
        show_desktop_pet_instance(&app, PetInstanceId::Primary, true)?;
    }

    let instance_id = {
        let state = app.state::<Mutex<DesktopPetState>>();
        let mut guard = state.lock().map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
        let id = guard.next_extra_id;
        guard.next_extra_id = guard.next_extra_id.saturating_add(1).max(1);
        PetInstanceId::Extra(id)
    };

    show_desktop_pet_instance(&app, instance_id, false)?;
    let status = get_desktop_pet_status(app)?;
    eprintln!(
        "[desktop-pet] extra pet opened visible={}, instances={}",
        status.visible, status.instance_count
    );
    Ok(status)
}

#[tauri::command]
pub async fn hide_desktop_pet(app: AppHandle, source: Option<String>) -> LocalResult<bool> {
    eprintln!(
        "[desktop-pet] hide command received source={}",
        source.as_deref().unwrap_or("unknown")
    );
    hide_desktop_pet_inner(&app)
}

fn hide_desktop_pet_inner(app: &AppHandle) -> LocalResult<bool> {
    let state = app.state::<Mutex<DesktopPetState>>();
    let instances = {
        let mut guard = state.lock().map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
        guard
            .instances
            .drain(..)
            .map(|mut instance| {
                instance.worker = None;
                (
                    instance.id,
                    instance.window.take(),
                    instance.stop_signal.take(),
                )
            })
            .collect::<Vec<_>>()
    };

    for (instance_id, window, stop_signal) in instances {
        if let Some(signal) = stop_signal {
            signal.store(true, Ordering::Relaxed);
        }

        if let Some(window) = window {
            if instance_id == PetInstanceId::Primary {
                persist_pet_window_position(&app, &window).ok();
            }
            window.close().map_err(|err| err.to_string())?;
        }
    }

    Ok(false)
}

#[tauri::command]
pub fn get_desktop_pet_status(app: AppHandle) -> LocalResult<DesktopPetStatus> {
    let state = app.state::<Mutex<DesktopPetState>>();
    let guard = state.lock().map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
    let instance_count = guard
        .instances
        .iter()
        .filter(|instance| {
            instance
                .window
                .as_ref()
                .is_some_and(|window| window.is_visible().unwrap_or(false))
        })
        .count();
    Ok(DesktopPetStatus {
        visible: instance_count > 0,
        instance_count,
    })
}

#[tauri::command]
pub fn start_desktop_pet_drag(app: AppHandle) -> LocalResult<()> {
    let state = app.state::<Mutex<DesktopPetState>>();
    let guard = state.lock().map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
    if let Some(window) = guard
        .instances
        .iter()
        .find(|instance| instance.id == PetInstanceId::Primary)
        .or_else(|| guard.instances.first())
        .and_then(|instance| instance.window.as_ref())
    {
        window.start_dragging().map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn create_pet_window(
    app: &AppHandle,
    visual_state: &PetVisualState,
    instance_id: PetInstanceId,
    interaction_signal: &Arc<AtomicU64>,
    persist_position: bool,
) -> LocalResult<Window> {
    let position = resolve_pet_position(app, visual_state, instance_id, persist_position)?;
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
    window.set_ignore_cursor_events(false).ok();

    #[cfg(windows)]
    {
        windows_pet_renderer::prepare_window(&window, interaction_signal)?;
    }
    #[cfg(target_os = "macos")]
    {
        let window_clone = window.clone();
        let signal = interaction_signal.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        window.run_on_main_thread(move || {
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
    }

    Ok(window)
}

fn configure_pet_window(
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
        let position = resolve_pet_position(app, visual_state, instance_id, persist_position)?;
        window
            .set_position(position)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn ensure_worker(
    app: &AppHandle,
    window: &Window,
    action: PetAction,
    instance_id: PetInstanceId,
) -> LocalResult<PetWorker> {
    let state = app.state::<Mutex<DesktopPetState>>();
    let interaction_signal = {
        let mut guard = state.lock().map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
        let instance = get_or_create_instance(&mut guard, instance_id);
        if let Some(worker) = instance.worker.clone() {
            return Ok(worker);
        }
        instance.interaction_signal.clone()
    };

    let action_state = Arc::new(Mutex::new(action));
    let bubble_state = Arc::new(Mutex::new(None));
    let stop_signal = Arc::new(AtomicBool::new(false));
    let worker = PetWorker {
        action: action_state.clone(),
    };
    let app_for_thread = app.clone();
    let window_for_thread = window.clone();
    let stop_for_thread = stop_signal.clone();
    let frames = load_pet_animation_frames(app)?;

    std::thread::Builder::new()
        .name(format!("liberty-desktop-pet-{}", instance_id.log_name()))
        .spawn(move || {
            run_pet_worker(
                app_for_thread,
                window_for_thread,
                instance_id,
                !matches!(instance_id, PetInstanceId::Extra(_)),
                action_state,
                bubble_state,
                stop_for_thread,
                interaction_signal,
                frames,
            );
        })
        .map_err(|err| err.to_string())?;

    let mut guard = state.lock().map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
    let instance = get_or_create_instance(&mut guard, instance_id);
    instance.stop_signal = Some(stop_signal);
    Ok(worker)
}

impl PetWorker {
    fn set_action(&self, action: PetAction) {
        if let Ok(mut guard) = self.action.lock() {
            *guard = action;
        }
    }
}

fn run_pet_worker(
    app: AppHandle,
    window: Window,
    instance_id: PetInstanceId,
    persist_position: bool,
    action_state: Arc<Mutex<PetAction>>,
    bubble_state: Arc<Mutex<Option<PetBubble>>>,
    stop_signal: Arc<AtomicBool>,
    interaction_signal: Arc<AtomicU64>,
    frames: PetAnimationFrames,
) {
    let mut frame_index = 0usize;
    let mut last_action = PetAction::Slack;
    let mut last_refresh = SystemTime::now();
    let mut last_proactive_bubble = SystemTime::now();
    let mut first_frame_logged = false;
    let mut handled_interactions = interaction_signal.load(Ordering::Relaxed);

    while !stop_signal.load(Ordering::Relaxed) {
        let pending_interactions = interaction_signal.load(Ordering::Relaxed);
        if pending_interactions != handled_interactions {
            handled_interactions = pending_interactions;
            handle_desktop_pet_interaction(&app, &action_state, &bubble_state);
        }

        if last_refresh.elapsed().unwrap_or_default() >= Duration::from_millis(STATE_REFRESH_MS) {
            if let Ok(settings) = local_db::get_pet_settings(&app) {
                if !settings.desktop_enabled {
                    let app_clone = app.clone();
                    app.run_on_main_thread(move || {
                        if let Err(error) = hide_desktop_pet_inner(&app_clone) {
                            eprintln!("[desktop-pet] failed to hide disabled pet: {error}");
                        }
                    })
                    .ok();
                    break;
                }

                if let Ok(visual_state) = resolve_visual_state(&app, &settings) {
                    if let Ok(mut guard) = action_state.lock() {
                        *guard = visual_state.action;
                    }
                    let app_clone = app.clone();
                    let window_clone = window.clone();
                    window
                        .run_on_main_thread(move || {
                            if let Err(error) =
                                configure_pet_window(&app_clone, &window_clone, &visual_state, instance_id, persist_position)
                            {
                                eprintln!("[desktop-pet] failed to configure window: {error}");
                            }
                        })
                        .ok();
                }
            }
            last_refresh = SystemTime::now();
        }

        if let Ok(settings) = local_db::get_pet_settings(&app) {
            if should_show_proactive_bubble(&settings, last_proactive_bubble) {
                let line = select_proactive_dialogue(&app);
                if let Ok(mut guard) = bubble_state.lock() {
                    *guard = Some(PetBubble {
                        text: line.clone(),
                        expires_at: SystemTime::now() + Duration::from_millis(BUBBLE_VISIBLE_MS),
                    });
                }
                last_proactive_bubble = SystemTime::now();
                eprintln!("[desktop-pet] proactive bubble text={line}");
            }
        }

        let action = action_state.lock().map(|guard| *guard).unwrap_or(PetAction::Slack);
        let bubble_text = current_bubble_text(&bubble_state);
        let bubble_theme = resolve_bubble_theme(&app, &window);
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
            window
                .run_on_main_thread(move || {
                    if let Err(error) =
                        windows_pet_renderer::paint_window(&window_clone, &frame_path, bubble_text.as_deref(), bubble_theme)
                    {
                        eprintln!("[desktop-pet] failed to paint window: {error}");
                    } else if should_log_frame {
                        eprintln!("[desktop-pet] painted first frame {}", frame_for_log.display());
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
            window
                .run_on_main_thread(move || {
                    if let Err(error) =
                        macos_pet_renderer::paint_window(&window_clone, &frame_path, bubble_text.as_deref(), bubble_theme)
                    {
                        eprintln!("[desktop-pet] failed to paint window: {error}");
                    } else if should_log_frame {
                        eprintln!("[desktop-pet] painted first frame {}", frame_for_log.display());
                    }
                })
                .ok();
            first_frame_logged = true;
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (&window, &frame_path);
        }

        frame_index = frame_index.saturating_add(1);
        std::thread::sleep(Duration::from_millis(ANIMATION_FRAME_MS));
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

fn get_or_create_instance(state: &mut DesktopPetState, instance_id: PetInstanceId) -> &mut DesktopPetInstance {
    if let Some(index) = state.instances.iter().position(|instance| instance.id == instance_id) {
        return &mut state.instances[index];
    }

    state.instances.push(DesktopPetInstance {
        id: instance_id,
        window: None,
        worker: None,
        stop_signal: None,
        interaction_signal: Arc::new(AtomicU64::new(0)),
    });
    state.instances.last_mut().expect("desktop pet instance was just inserted")
}

fn handle_desktop_pet_interaction(
    app: &AppHandle,
    action_state: &Arc<Mutex<PetAction>>,
    bubble_state: &Arc<Mutex<Option<PetBubble>>>,
) {
    let line = select_interaction_dialogue(app);
    eprintln!("[desktop-pet] interaction action=tap");

    if let Err(error) = local_db::apply_pet_growth_event(
        app,
        "interaction",
        "tap",
        1,
        "cheerful",
        Some(line.as_str()),
    ) {
        eprintln!("[desktop-pet] interaction ledger skipped: {error}");
    }

    if let Ok(mut guard) = action_state.lock() {
        *guard = PetAction::Toy;
    }
    if let Ok(mut guard) = bubble_state.lock() {
        *guard = Some(PetBubble {
            text: line.clone(),
            expires_at: SystemTime::now() + Duration::from_millis(BUBBLE_VISIBLE_MS),
        });
    }
    eprintln!("[desktop-pet] bubble text={line}");
}

fn current_bubble_text(bubble_state: &Arc<Mutex<Option<PetBubble>>>) -> Option<String> {
    let mut guard = bubble_state.lock().ok()?;
    let bubble = guard.as_ref()?;
    if SystemTime::now() >= bubble.expires_at {
        *guard = None;
        return None;
    }
    Some(bubble.text.clone())
}

fn select_interaction_dialogue(app: &AppHandle) -> String {
    let _ = app;
    let lines = [
        "我听见你叫我了，我就在旁边。",
        "你点到我啦，我会继续陪着你。",
        "这一点我收到啦，接下来我会更专心。",
        "我刚好在看着你，你忙你的，我在。",
        "你一碰我就醒神了，我会把节奏守好。",
        "我被你点到啦，今天也会认真陪你。",
        "我在这里呢，你回头看我时我都会回应。",
        "这一声招呼我听清了，我不会走开。",
    ];
    let index = (now_ms() as usize) % lines.len();
    lines[index].to_string()
}

fn should_show_proactive_bubble(settings: &PetSettings, last_proactive_bubble: SystemTime) -> bool {
    if settings.muted || settings.focus_mode_enabled || settings.proactive_level <= 0 {
        return false;
    }

    let interval_ms = match settings.proactive_level {
        3 => 20_000,
        2 => 35_000,
        _ => 55_000,
    };
    last_proactive_bubble.elapsed().unwrap_or_default() >= Duration::from_millis(interval_ms)
}

fn select_proactive_dialogue(app: &AppHandle) -> String {
    let profile = local_db::get_pet_profile(app).ok();
    let name = profile
        .as_ref()
        .map(|value| value.name.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("Libby");
    let lines = [
        format!("{name} 在这边陪着你，先专心做手头这件事。"),
        format!("{name} 看你还在忙，我就安静待在旁边。"),
        format!("节奏我帮你守着，{name} 不会走开。"),
        format!("先把这一段做完，{name} 等你回头。"),
        format!("{name} 还醒着，有事随时点我一下。"),
        format!("你继续推进，我在桌面上给你盯着状态。"),
    ];
    let index = (now_ms() as usize / 7) % lines.len();
    lines[index].clone()
}

fn resolve_visual_state(app: &AppHandle, settings: &PetSettings) -> LocalResult<PetVisualState> {
    let profile = local_db::get_pet_profile(app)?;
    let jobs = local_db::list_jobs(app)?;
    let latest_event = local_db::list_pet_event_ledger(app, 1)?.into_iter().next();
    let environment_state = jobs
        .iter()
        .map(|job| job.overall_status.as_str())
        .find(|status| matches!(*status, "transcribing" | "speaker_processing" | "summarizing" | "queued"));

    Ok(PetVisualState {
        action: resolve_pet_action(environment_state, latest_event.as_ref(), &profile.current_mood),
        always_on_top: settings.always_on_top,
        last_window_x: settings.last_window_x,
        last_window_y: settings.last_window_y,
    })
}

fn resolve_pet_action(
    environment_state: Option<&str>,
    latest_event: Option<&PetEventLedgerEntry>,
    mood: &str,
) -> PetAction {
    if let Some(environment_state) = environment_state {
        return get_action_for_environment(environment_state, mood);
    }

    let latest_event_at = latest_event
        .and_then(|event| chrono::DateTime::parse_from_rfc3339(&event.event_time).ok())
        .map(|value| value.timestamp_millis().max(0) as u64)
        .unwrap_or(0);
    let now_ms = now_ms();
    let idle_ms = if latest_event_at > 0 {
        now_ms.saturating_sub(latest_event_at)
    } else {
        0
    };

    if let Some(event) = latest_event {
        if idle_ms <= RECENT_EVENT_ACTION_HOLD_MS {
            return get_action_for_recent_event(event, mood);
        }
    }

    if latest_event_at == 0 {
        return get_daily_idle_action(0, now_ms);
    }

    if idle_ms >= NEEDY_AFTER_MS {
        return get_daily_idle_action(latest_event_at, idle_ms);
    }

    get_action_for_mood(mood)
}

fn get_action_for_environment(environment_state: &str, mood: &str) -> PetAction {
    match environment_state {
        "queued" => PetAction::Slack,
        "transcribing" | "speaker_processing" | "summarizing" => PetAction::Work,
        "completed" => PetAction::Snow,
        "failed" => PetAction::Pants,
        "uploaded" => PetAction::Read,
        _ => get_action_for_mood(mood),
    }
}

fn get_action_for_mood(mood: &str) -> PetAction {
    match mood {
        "cheerful" => PetAction::Snow,
        "excited" => PetAction::Run,
        "proud" => PetAction::Eat,
        "needy" => PetAction::Toy,
        "sleepy" => PetAction::Sleep,
        "bored" => PetAction::Read,
        _ => PetAction::Slack,
    }
}

fn get_action_for_recent_event(event: &PetEventLedgerEntry, mood: &str) -> PetAction {
    if event.event_type == "interaction" {
        return match event.event_source.as_str() {
            "feed" => PetAction::Eat,
            "encourage" => PetAction::Rope,
            "pet" => PetAction::Crush,
            "tap" => PetAction::Toy,
            _ => get_action_for_mood(mood),
        };
    }

    match event.event_type.as_str() {
        "job_created" => PetAction::Drive,
        "transcription_started" => PetAction::Work,
        "transcription_completed" | "ai_summary_completed" | "export_completed" => PetAction::Snow,
        "daily_open" => PetAction::Slack,
        _ => get_action_for_mood(mood),
    }
}

fn get_daily_idle_action(latest_event_at: u64, idle_ms: u64) -> PetAction {
    const ACTIONS: [PetAction; 13] = [
        PetAction::Slack,
        PetAction::Toy,
        PetAction::Rope,
        PetAction::Drive,
        PetAction::Crush,
        PetAction::Defecate,
        PetAction::Eat,
        PetAction::Pants,
        PetAction::Read,
        PetAction::Run,
        PetAction::Sleep,
        PetAction::Snow,
        PetAction::Work,
    ];
    let bucket = ((latest_event_at + idle_ms) / DAILY_ACTION_BUCKET_MS) as usize;
    ACTIONS[bucket % ACTIONS.len()]
}

impl PetAnimationFrames {
    fn frame_path(&self, action: PetAction, frame_index: usize) -> LocalResult<&PathBuf> {
        let frames = match action {
            PetAction::Crush => &self.crush,
            PetAction::Defecate => &self.defecate,
            PetAction::Drive => &self.drive,
            PetAction::Eat => &self.eat,
            PetAction::Pants => &self.pants,
            PetAction::Read => &self.read,
            PetAction::Rope => &self.rope,
            PetAction::Run => &self.run,
            PetAction::Slack => &self.slack,
            PetAction::Sleep => &self.sleep,
            PetAction::Snow => &self.snow,
            PetAction::Toy => &self.toy,
            PetAction::Work => &self.work,
        };
        if frames.is_empty() {
            return Err(format!("宠物资源组为空：{}", action.as_str()));
        }
        Ok(&frames[frame_index % frames.len()])
    }
}

fn load_pet_animation_frames(app: &AppHandle) -> LocalResult<PetAnimationFrames> {
    Ok(PetAnimationFrames {
        crush: resolve_pet_frame_paths(app, PetAction::Crush)?,
        defecate: resolve_pet_frame_paths(app, PetAction::Defecate)?,
        drive: resolve_pet_frame_paths(app, PetAction::Drive)?,
        eat: resolve_pet_frame_paths(app, PetAction::Eat)?,
        pants: resolve_pet_frame_paths(app, PetAction::Pants)?,
        read: resolve_pet_frame_paths(app, PetAction::Read)?,
        rope: resolve_pet_frame_paths(app, PetAction::Rope)?,
        run: resolve_pet_frame_paths(app, PetAction::Run)?,
        slack: resolve_pet_frame_paths(app, PetAction::Slack)?,
        sleep: resolve_pet_frame_paths(app, PetAction::Sleep)?,
        snow: resolve_pet_frame_paths(app, PetAction::Snow)?,
        toy: resolve_pet_frame_paths(app, PetAction::Toy)?,
        work: resolve_pet_frame_paths(app, PetAction::Work)?,
    })
}

fn resolve_pet_frame_paths(app: &AppHandle, action: PetAction) -> LocalResult<Vec<PathBuf>> {
    sorted_png_frames(pet_resource_root(app)?.join(action.as_str()))
}

fn sorted_png_frames(group_dir: PathBuf) -> LocalResult<Vec<PathBuf>> {
    let mut entries = std::fs::read_dir(&group_dir)
        .map_err(|err| format!("读取宠物资源失败 {}: {err}", group_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
        .filter(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<u32>().ok())
                .is_some()
        })
        .collect::<Vec<_>>();

    entries.sort_by_key(|path| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    Ok(entries)
}

fn pet_resource_root(app: &AppHandle) -> LocalResult<PathBuf> {
    let resource_dir = app.path().resource_dir().map_err(|err| err.to_string())?;
    let packaged = resource_dir.join("pet");
    if packaged.exists() {
        return Ok(packaged);
    }

    if let Ok(current_dir) = std::env::current_dir() {
        let dev_candidates = [
            current_dir.join("resources/pet"),
            current_dir.join("src-tauri/resources/pet"),
            current_dir.join("apps/desktop/src-tauri/resources/pet"),
        ];
        for candidate in dev_candidates {
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Ok(app
        .path()
        .resolve(
            "resources/pet",
            tauri::path::BaseDirectory::Resource,
        )
        .unwrap_or(resource_dir.join("pet")))
}

fn resolve_pet_position(
    app: &AppHandle,
    visual_state: &PetVisualState,
    instance_id: PetInstanceId,
    persist_position: bool,
) -> LocalResult<LogicalPosition<f64>> {
    if persist_position {
        if let (Some(x), Some(y)) = (visual_state.last_window_x, visual_state.last_window_y) {
            if is_position_visible(app, x, y)? {
                return Ok(LogicalPosition::new(x, y));
            }
        }
    }

    let mut position = default_pet_position(app)?;
    if let PetInstanceId::Extra(id) = instance_id {
        let column = ((id - 1) % 3) as f64;
        let row = (((id - 1) / 3) % 3) as f64;
        position.x -= EXTRA_PET_OFFSET_X * (column + 1.0);
        position.y -= EXTRA_PET_OFFSET_Y * row;
        if is_position_visible(app, position.x, position.y)? {
            return Ok(position);
        }
    }

    Ok(position)
}

fn default_pet_position(app: &AppHandle) -> LocalResult<LogicalPosition<f64>> {
    let monitor = app
        .primary_monitor()
        .map_err(|err| err.to_string())?
        .or_else(|| app.available_monitors().ok().and_then(|monitors| monitors.into_iter().next()));

    let Some(monitor) = monitor else {
        return Ok(LogicalPosition::new(80.0, 80.0));
    };

    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    Ok(LogicalPosition::new(
        (work_area.position.x as f64 + work_area.size.width as f64 - PET_WINDOW_WIDTH * scale - 24.0) / scale,
        (work_area.position.y as f64 + work_area.size.height as f64 - PET_WINDOW_HEIGHT * scale - 24.0) / scale,
    ))
}

fn is_position_visible(app: &AppHandle, x: f64, y: f64) -> LocalResult<bool> {
    let monitors = app.available_monitors().map_err(|err| err.to_string())?;
    Ok(monitors.into_iter().any(|monitor| {
        let scale = monitor.scale_factor();
        let work_area = monitor.work_area();
        let px = x * scale;
        let py = y * scale;
        let max_x = work_area.position.x as f64 + work_area.size.width as f64 - PET_WINDOW_WIDTH * scale;
        let max_y = work_area.position.y as f64 + work_area.size.height as f64 - PET_WINDOW_HEIGHT * scale;
        px >= work_area.position.x as f64
            && py >= work_area.position.y as f64
            && px <= max_x
            && py <= max_y
    }))
}

fn attach_window_position_persistence(app: &AppHandle, window: &Window) {
    let app = app.clone();
    let window_for_event = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Moved(_)) {
            persist_pet_window_position(&app, &window_for_event).ok();
        }
    });
}

fn persist_pet_window_position(app: &AppHandle, window: &Window) -> LocalResult<()> {
    let position = window.outer_position().map_err(|err| err.to_string())?;
    let scale = window.scale_factor().map_err(|err| err.to_string())?;
    let mut settings = local_db::get_pet_settings(app)?;
    settings.last_window_x = Some(position.x as f64 / scale);
    settings.last_window_y = Some(position.y as f64 / scale);
    settings.updated_at = Utc::now().to_rfc3339();
    local_db::save_pet_settings(app, &settings)?;
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(target_os = "macos")]
mod macos_pet_renderer {
    use super::*;
    use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSColor, NSEvent, NSFloatingWindowLevel, NSFont, NSImage, NSImageScaling, NSImageView,
        NSTextAlignment, NSTextField, NSView, NSWindow, NSWindowCollectionBehavior,
    };
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::NSString;
    use objc2_quartz_core::CALayer;

    const BUBBLE_LAYER_NAME: &str = "LibertyDesktopPetBubbleLayer";

    define_class!(
        #[unsafe(super(NSView))]
        #[name = "LibertyDesktopPetContentView"]
        #[thread_kind = MainThreadOnly]
        #[ivars = Arc<AtomicU64>]
        struct PetContentView;

        impl PetContentView {
            #[unsafe(method(mouseDown:))]
            fn mouse_down(&self, event: &NSEvent) {
                self.ivars().fetch_add(1, Ordering::Relaxed);
                if let Some(window) = event.window(MainThreadMarker::from(self)) {
                    window.performWindowDragWithEvent(event);
                }
            }
        }
    );

    unsafe impl objc2_foundation::NSObjectProtocol for PetContentView {}

    impl PetContentView {
        fn new(mtm: MainThreadMarker, frame: CGRect, interaction_signal: Arc<AtomicU64>) -> objc2::rc::Retained<Self> {
            let this = mtm.alloc().set_ivars(interaction_signal);
            unsafe { msg_send![super(this), initWithFrame: frame] }
        }
    }

    pub fn prepare_window(window: &Window, interaction_signal: Arc<AtomicU64>) -> LocalResult<()> {
        let mtm = MainThreadMarker::new().ok_or_else(|| "macOS 桌宠初始化必须在主线程执行。".to_string())?;
        let ns_window = native_window(window)?;
        ns_window.setOpaque(false);
        ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
        ns_window.setAlphaValue(1.0);
        ns_window.setLevel(NSFloatingWindowLevel);
        ns_window.setMovable(true);
        ns_window.setMovableByWindowBackground(true);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Transient,
        );
        let content_view = PetContentView::new(
            mtm,
            CGRect::new(
                CGPoint::new(0.0, 0.0),
                CGSize::new(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT),
            ),
            interaction_signal,
        );
        content_view.setWantsLayer(true);
        ns_window.setContentView(Some(&content_view));
        ns_window.setIgnoresMouseEvents(false);
        ns_window.orderFrontRegardless();
        Ok(())
    }

    pub fn paint_window(
        window: &Window,
        frame_path: &PathBuf,
        bubble_text: Option<&str>,
        bubble_theme: PetBubbleTheme,
    ) -> LocalResult<()> {
        let mtm = MainThreadMarker::new().ok_or_else(|| "macOS 桌宠绘制必须在主线程执行。".to_string())?;
        let ns_window = native_window(window)?;
        let frame_path = frame_path
            .to_str()
            .ok_or_else(|| format!("宠物图片路径不是有效 UTF-8：{}", frame_path.display()))?;
        let image_path = NSString::from_str(frame_path);
        let image = NSImage::initWithContentsOfFile(NSImage::alloc(), &image_path)
            .ok_or_else(|| format!("读取宠物图片失败：{frame_path}"))?;

        ns_window.setOpaque(false);
        ns_window.setBackgroundColor(Some(&NSColor::clearColor()));

        let content_view = ns_window.contentView().ok_or_else(|| "macOS 桌宠内容视图为空。".to_string())?;
        let image_view = match content_view
            .subviews()
            .into_iter()
            .find_map(|view| view.downcast::<NSImageView>().ok())
        {
            Some(image_view) => image_view,
            None => {
                let view = create_image_view(mtm);
                content_view.addSubview(&view);
                view
            }
        };

        image_view.setFrame(CGRect::new(
            CGPoint::new((PET_WINDOW_WIDTH - PET_SPRITE_WIDTH as f64) / 2.0, 6.0),
            CGSize::new(PET_SPRITE_WIDTH as f64, PET_SPRITE_HEIGHT as f64),
        ));
        image_view.setImage(Some(&image));

        update_bubble_label(mtm, &content_view, bubble_text, bubble_theme);

        image_view.displayIfNeeded();
        content_view.setNeedsDisplay(true);
        ns_window.displayIfNeeded();

        Ok(())
    }

    fn create_image_view(mtm: MainThreadMarker) -> objc2::rc::Retained<NSImageView> {
        let view = NSImageView::initWithFrame(
            NSImageView::alloc(mtm),
            CGRect::new(
                CGPoint::new((PET_WINDOW_WIDTH - PET_SPRITE_WIDTH as f64) / 2.0, 6.0),
                CGSize::new(PET_SPRITE_WIDTH as f64, PET_SPRITE_HEIGHT as f64),
            ),
        );
        view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        view
    }

    fn update_bubble_label(
        mtm: MainThreadMarker,
        content_view: &NSView,
        bubble_text: Option<&str>,
        bubble_theme: PetBubbleTheme,
    ) {
        let label = match content_view
            .subviews()
            .into_iter()
            .find_map(|view| view.downcast::<NSTextField>().ok())
        {
            Some(label) => label,
            None => {
                let initial = NSString::from_str("");
                let label = NSTextField::wrappingLabelWithString(&initial, mtm);
                label.setDrawsBackground(false);
                label.setBordered(false);
                label.setBezeled(false);
                label.setEditable(false);
                label.setSelectable(false);
                label.setWantsLayer(true);
                label.setMaximumNumberOfLines(2);
                label.setAlignment(NSTextAlignment::Center);
                label.setFont(Some(&NSFont::systemFontOfSize(13.5)));
                content_view.addSubview(&label);
                label
            }
        };
        let (background, foreground) = if bubble_theme.is_dark() {
            (
                NSColor::colorWithCalibratedWhite_alpha(0.10, 0.90),
                NSColor::colorWithCalibratedWhite_alpha(0.94, 1.0),
            )
        } else {
            (
                NSColor::colorWithCalibratedWhite_alpha(1.0, 0.94),
                NSColor::colorWithCalibratedWhite_alpha(0.10, 1.0),
            )
        };
        label.setDrawsBackground(false);
        label.setBackgroundColor(Some(&NSColor::clearColor()));
        label.setTextColor(Some(&foreground));

        if let Some(text) = bubble_text.filter(|value| !value.trim().is_empty()) {
            apply_bubble_layer_style(content_view, &background, bubble_theme);
            if let Some(label_layer) = label.layer() {
                label_layer.setZPosition(2.0);
            }

            let text = NSString::from_str(text);
            label.setStringValue(&text);
            label.setFrame(text_frame_for_bubble(text.length() as usize));
            label.setHidden(false);
        } else {
            label.setHidden(true);
            set_bubble_layer_hidden(content_view, true);
        }
    }

    fn text_frame_for_bubble(character_count: usize) -> CGRect {
        let line_count = if character_count > 16 { 2.0 } else { 1.0 };
        let line_height = 17.0;
        let text_height = line_count * line_height;
        let bubble_y = 156.0;
        let bubble_height = 58.0;
        CGRect::new(
            CGPoint::new(34.0, bubble_y + (bubble_height - text_height) / 2.0),
            CGSize::new(PET_WINDOW_WIDTH - 68.0, text_height),
        )
    }

    fn apply_bubble_layer_style(content_view: &NSView, background: &NSColor, bubble_theme: PetBubbleTheme) {
        content_view.setWantsLayer(true);
        let Some(root_layer) = content_view.layer() else {
            return;
        };
        let layer = find_or_create_bubble_layer(&root_layer);
        let border = if bubble_theme.is_dark() {
            NSColor::colorWithCalibratedWhite_alpha(1.0, 0.14)
        } else {
            NSColor::colorWithCalibratedWhite_alpha(0.0, 0.10)
        };
        let shadow = NSColor::colorWithCalibratedWhite_alpha(0.0, if bubble_theme.is_dark() { 0.34 } else { 0.18 });
        layer.setFrame(CGRect::new(
            CGPoint::new(20.0, 156.0),
            CGSize::new(PET_WINDOW_WIDTH - 40.0, 58.0),
        ));
        layer.setZPosition(1.0);
        layer.setBackgroundColor(Some(&background.CGColor()));
        layer.setCornerRadius(14.0);
        layer.setMasksToBounds(false);
        layer.setBorderWidth(1.0);
        layer.setBorderColor(Some(&border.CGColor()));
        layer.setShadowColor(Some(&shadow.CGColor()));
        layer.setShadowOpacity(if bubble_theme.is_dark() { 0.30 } else { 0.18 });
        layer.setShadowRadius(10.0);
        layer.setShadowOffset(CGSize::new(0.0, -2.0));
        layer.setHidden(false);
    }

    fn set_bubble_layer_hidden(content_view: &NSView, hidden: bool) {
        let Some(root_layer) = content_view.layer() else {
            return;
        };
        let layer_name = NSString::from_str(BUBBLE_LAYER_NAME);
        if let Some(layer) = unsafe { root_layer.sublayers() }.and_then(|layers| {
            layers.into_iter().find(|layer| {
                layer
                    .name()
                    .is_some_and(|name| name.isEqualToString(&layer_name))
            })
        }) {
            layer.setHidden(hidden);
        }
    }

    fn find_or_create_bubble_layer(root_layer: &CALayer) -> objc2::rc::Retained<CALayer> {
        let layer_name = NSString::from_str(BUBBLE_LAYER_NAME);
        if let Some(layer) = unsafe { root_layer.sublayers() }.and_then(|layers| {
            layers.into_iter().find(|layer| {
                layer
                    .name()
                    .is_some_and(|name| name.isEqualToString(&layer_name))
            })
        }) {
            return layer;
        }

        let layer = CALayer::layer();
        layer.setName(Some(&layer_name));
        root_layer.addSublayer(&layer);
        layer
    }

    fn native_window(window: &Window) -> LocalResult<&'static NSWindow> {
        let ns_window = window.ns_window().map_err(|err| err.to_string())?;
        if ns_window.is_null() {
            return Err("macOS 桌宠窗口句柄为空。".into());
        }

        Ok(unsafe { &*(ns_window.cast::<NSWindow>()) })
    }
}

#[cfg(windows)]
mod windows_pet_renderer {
    use super::*;
    use std::sync::atomic::AtomicU64;

    pub fn prepare_window(window: &Window, interaction_signal: &Arc<AtomicU64>) -> LocalResult<()> {
        set_native_window_style(window, interaction_signal)
    }

    pub fn paint_window(
        window: &Window,
        frame_path: &PathBuf,
        bubble_text: Option<&str>,
        bubble_theme: PetBubbleTheme,
    ) -> LocalResult<()> {
        let image = image::open(frame_path)
            .map_err(|err| format!("读取宠物图片失败 {}: {err}", frame_path.display()))?
            .to_rgba8();
        let (source_width, source_height) = image.dimensions();
        let scale = window.scale_factor().map_err(|err| err.to_string())?;
        let window_width = (PET_WINDOW_WIDTH * scale).round().max(1.0) as u32;
        let window_height = (PET_WINDOW_HEIGHT * scale).round().max(1.0) as u32;
        let sprite_width = (PET_SPRITE_WIDTH as f64 * scale).round().max(1.0) as u32;
        let sprite_height = (PET_SPRITE_HEIGHT as f64 * scale).round().max(1.0) as u32;
        let mut buffer = vec![0u8; (window_width as usize) * (window_height as usize) * 4];
        let target_x = window_width.saturating_sub(sprite_width) / 2;
        let target_y = window_height.saturating_sub(sprite_height + (6.0 * scale).round() as u32);

        for y in 0..sprite_height {
            for x in 0..sprite_width {
                let source_x = x * source_width / sprite_width;
                let source_y = y * source_height / sprite_height;
                let pixel = image.get_pixel(source_x, source_y).0;
                let destination_x = target_x + x;
                let destination_y = target_y + y;
                let destination_index = ((destination_y * window_width + destination_x) * 4) as usize;
                let alpha = u16::from(pixel[3]);

                buffer[destination_index] = ((u16::from(pixel[2]) * alpha + 127) / 255) as u8;
                buffer[destination_index + 1] = ((u16::from(pixel[1]) * alpha + 127) / 255) as u8;
                buffer[destination_index + 2] = ((u16::from(pixel[0]) * alpha + 127) / 255) as u8;
                buffer[destination_index + 3] = pixel[3];
            }
        }

        if bubble_text.is_some_and(|value| !value.trim().is_empty()) {
            draw_bubble(&mut buffer, window_width, scale, bubble_theme);
        }

        let _ = window.set_size(tauri::LogicalSize::new(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT));
        paint_layered_window(
            window,
            &buffer,
            window_width as i32,
            window_height as i32,
            scale,
            bubble_text,
            bubble_theme,
        )
    }

    fn set_native_window_style(window: &Window, interaction_signal: &Arc<AtomicU64>) -> LocalResult<()> {
        let hwnd = window.hwnd().map_err(|err| err.to_string())?;
        unsafe {
            use windows_sys::Win32::UI::{
                Shell::SetWindowSubclass,
                WindowsAndMessaging::{
                    GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_NOACTIVATE,
                    WS_EX_TOOLWINDOW,
                },
            };
            let hwnd = hwnd.0 as *mut std::ffi::c_void;
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                style | WS_EX_LAYERED as isize | WS_EX_TOOLWINDOW as isize | WS_EX_NOACTIVATE as isize,
            );
            SetWindowSubclass(
                hwnd,
                Some(pet_window_subclass_proc),
                1,
                Arc::as_ptr(interaction_signal) as usize,
            );
        }
        Ok(())
    }

    unsafe extern "system" fn pet_window_subclass_proc(
        hwnd: *mut std::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
        _subclass_id: usize,
        ref_data: usize,
    ) -> isize {
        use windows_sys::Win32::UI::{
            Shell::DefSubclassProc,
            WindowsAndMessaging::{HTCLIENT, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_NCHITTEST, WM_NCLBUTTONUP},
        };

        if msg == WM_NCHITTEST {
            return HTCLIENT as isize;
        }
        if msg == WM_LBUTTONDOWN {
            begin_window_drag(hwnd);
        }
        if matches!(msg, WM_LBUTTONUP | WM_NCLBUTTONUP) && ref_data != 0 {
            let signal = &*(ref_data as *const AtomicU64);
            signal.fetch_add(1, Ordering::Relaxed);
        }

        DefSubclassProc(hwnd, msg, wparam, lparam)
    }

    unsafe fn begin_window_drag(hwnd: *mut std::ffi::c_void) {
        use windows_sys::Win32::{
            Foundation::{POINT, POINTS},
            UI::{
                Input::KeyboardAndMouse::ReleaseCapture,
                WindowsAndMessaging::{GetCursorPos, PostMessageW, HTCAPTION, WM_NCLBUTTONDOWN},
            },
        };

        let mut cursor = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut cursor) == 0 {
            return;
        }

        let points = POINTS {
            x: cursor.x as i16,
            y: cursor.y as i16,
        };

        ReleaseCapture();
        PostMessageW(
            hwnd,
            WM_NCLBUTTONDOWN,
            HTCAPTION as usize,
            &points as *const POINTS as isize,
        );
    }

    fn draw_bubble(buffer: &mut [u8], width: u32, scale: f64, bubble_theme: PetBubbleTheme) {
        let bubble_x = (20.0 * scale).round() as u32;
        let bubble_y = (8.0 * scale).round() as u32;
        let bubble_width = width.saturating_sub((40.0 * scale).round() as u32).max(1);
        let bubble_height = (64.0 * scale).round().max(1.0) as u32;
        let shadow_y = bubble_y + (3.0 * scale).round() as u32;
        fill_rounded_rect(
            buffer,
            width,
            bubble_x + (2.0 * scale).round() as u32,
            shadow_y,
            bubble_width,
            bubble_height,
            (0, 0, 0, if bubble_theme.is_dark() { 82 } else { 38 }),
            (14.0 * scale).round().max(10.0) as u32,
        );
        let (fill, stroke) = if bubble_theme.is_dark() {
            ((24, 26, 31, 236), (255, 255, 255, 34))
        } else {
            ((255, 255, 255, 240), (36, 40, 48, 26))
        };
        fill_rounded_rect(
            buffer,
            width,
            bubble_x,
            bubble_y,
            bubble_width,
            bubble_height,
            fill,
            (14.0 * scale).round().max(10.0) as u32,
        );
        stroke_rounded_rect(
            buffer,
            width,
            bubble_x,
            bubble_y,
            bubble_width,
            bubble_height,
            stroke,
            (14.0 * scale).round().max(10.0) as u32,
        );
    }

    fn fill_rounded_rect(
        buffer: &mut [u8],
        width: u32,
        x: u32,
        y: u32,
        rect_width: u32,
        rect_height: u32,
        (r, g, b, a): (u8, u8, u8, u8),
        radius: u32,
    ) {
        for py in y..y.saturating_add(rect_height) {
            for px in x..x.saturating_add(rect_width) {
                let left = px.saturating_sub(x);
                let top = py.saturating_sub(y);
                let right = x.saturating_add(rect_width).saturating_sub(px + 1);
                let bottom = y.saturating_add(rect_height).saturating_sub(py + 1);
                let corner_dx = radius.saturating_sub(left.min(right).saturating_add(1));
                let corner_dy = radius.saturating_sub(top.min(bottom).saturating_add(1));
                if corner_dx > 0 && corner_dy > 0 && corner_dx * corner_dx + corner_dy * corner_dy > radius * radius {
                    continue;
                }

                let index = ((py * width + px) * 4) as usize;
                if index + 3 >= buffer.len() {
                    continue;
                }
                buffer[index] = ((u16::from(b) * u16::from(a) + 127) / 255) as u8;
                buffer[index + 1] = ((u16::from(g) * u16::from(a) + 127) / 255) as u8;
                buffer[index + 2] = ((u16::from(r) * u16::from(a) + 127) / 255) as u8;
                buffer[index + 3] = a;
            }
        }
    }

    fn stroke_rounded_rect(
        buffer: &mut [u8],
        width: u32,
        x: u32,
        y: u32,
        rect_width: u32,
        rect_height: u32,
        color: (u8, u8, u8, u8),
        radius: u32,
    ) {
        fill_rounded_rect(buffer, width, x, y, rect_width, 1, color, radius);
        fill_rounded_rect(
            buffer,
            width,
            x,
            y.saturating_add(rect_height.saturating_sub(1)),
            rect_width,
            1,
            color,
            radius,
        );
        fill_rounded_rect(buffer, width, x, y, 1, rect_height, color, radius);
        fill_rounded_rect(
            buffer,
            width,
            x.saturating_add(rect_width.saturating_sub(1)),
            y,
            1,
            rect_height,
            color,
            radius,
        );
    }

    fn paint_layered_window(
        window: &Window,
        bgra: &[u8],
        width: i32,
        height: i32,
        scale: f64,
        bubble_text: Option<&str>,
        bubble_theme: PetBubbleTheme,
    ) -> LocalResult<()> {
        unsafe {
            use windows_sys::Win32::{
                Foundation::POINT,
                Graphics::Gdi::{
                    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject,
                    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
                    DIB_RGB_COLORS,
                },
                UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA},
            };

            let hwnd = window.hwnd().map_err(|err| err.to_string())?.0 as *mut std::ffi::c_void;
            let position = window.outer_position().map_err(|err| err.to_string())?;
            let screen_position = POINT {
                x: position.x,
                y: position.y,
            };
            let size = windows_sys::Win32::Foundation::SIZE { cx: width, cy: height };
            let source_position = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let memory_dc = CreateCompatibleDC(std::ptr::null_mut());
            if memory_dc.is_null() {
                return Err("创建桌宠绘制 DC 失败。".into());
            }

            let mut bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [Default::default()],
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                memory_dc,
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() {
                DeleteDC(memory_dc);
                return Err("创建桌宠位图失败。".into());
            }

            std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast::<u8>(), bgra.len());
            let old_bitmap = SelectObject(memory_dc, bitmap);
            if let Some(text) = bubble_text.filter(|value| !value.trim().is_empty()) {
                draw_bubble_text(memory_dc, width, scale, text, bubble_theme);
            }
            let ok = UpdateLayeredWindow(
                hwnd,
                std::ptr::null_mut(),
                &screen_position,
                &size,
                memory_dc,
                &source_position,
                0,
                &blend,
                ULW_ALPHA,
            );
            SelectObject(memory_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(memory_dc);

            if ok == 0 {
                return Err("更新桌宠窗口失败。".into());
            }
        }

        Ok(())
    }

    unsafe fn draw_bubble_text(
        memory_dc: *mut std::ffi::c_void,
        width: i32,
        scale: f64,
        text: &str,
        bubble_theme: PetBubbleTheme,
    ) {
        use windows_sys::Win32::{
            Foundation::RECT,
            Graphics::Gdi::{
                DrawTextW, SetBkMode, SetTextColor, DT_CALCRECT, DT_CENTER, DT_EDITCONTROL, DT_WORDBREAK,
                TRANSPARENT,
            },
        };

        let left = (36.0 * scale).round() as i32;
        let right = width - left;
        let bubble_top = (8.0 * scale).round() as i32;
        let bubble_bottom = (72.0 * scale).round() as i32;
        let max_text_height = bubble_bottom - bubble_top - (18.0 * scale).round() as i32;
        let text_top_limit = bubble_top + (9.0 * scale).round() as i32;
        let text_bottom_limit = bubble_bottom - (9.0 * scale).round() as i32;
        let mut rect = RECT {
            left,
            top: text_top_limit,
            right,
            bottom: text_bottom_limit,
        };
        let mut wide = text.encode_utf16().collect::<Vec<_>>();
        wide.push(0);
        SetBkMode(memory_dc, TRANSPARENT as i32);
        SetTextColor(memory_dc, if bubble_theme.is_dark() { 0x00f2f0ec } else { 0x002a2622 });
        let mut measure_rect = rect;
        DrawTextW(
            memory_dc,
            wide.as_ptr(),
            -1,
            &mut measure_rect,
            DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_CALCRECT,
        );
        let measured_height = (measure_rect.bottom - measure_rect.top).clamp(1, max_text_height);
        let centered_top = bubble_top + ((bubble_bottom - bubble_top - measured_height) / 2);
        rect.top = centered_top;
        rect.bottom = centered_top + measured_height;
        DrawTextW(
            memory_dc,
            wide.as_ptr(),
            -1,
            &mut rect,
            DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL,
        );
    }
}
