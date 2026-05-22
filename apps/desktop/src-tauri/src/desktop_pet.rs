use crate::local_db::{self, LocalResult};
use chrono::Utc;
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::SystemTime,
};
use tauri::{AppHandle, Manager, Runtime, Window};

use self::{
    behavior::{
        attach_window_position_persistence, persist_pet_window_position, resolve_visual_state,
    },
    worker::{configure_pet_window, create_pet_window, ensure_worker, get_or_create_instance},
};

const PET_WINDOW_LABEL: &str = "desktop-pet";
const EXTRA_PET_WINDOW_LABEL_PREFIX: &str = "desktop-pet-extra";
pub(crate) const PET_WINDOW_WIDTH: f64 = 320.0;
pub(crate) const PET_WINDOW_HEIGHT: f64 = 220.0;
#[cfg(any(windows, target_os = "macos"))]
pub(crate) const PET_SPRITE_WIDTH: u32 = 148;
#[cfg(any(windows, target_os = "macos"))]
pub(crate) const PET_SPRITE_HEIGHT: u32 = 148;
pub(crate) const ANIMATION_FRAME_MS: u64 = 1000;
pub(crate) const PET_RENDER_TICK_MS: u64 = 100;
pub(crate) const STATE_REFRESH_MS: u64 = 5000;
pub(crate) const RECENT_EVENT_ACTION_HOLD_MS: u64 = 45_000;
pub(crate) const NEEDY_AFTER_MS: u64 = 30_000;
pub(crate) const DAILY_ACTION_BUCKET_MS: u64 = 120_000;
pub(crate) const BUBBLE_VISIBLE_MS: u64 = 7_000;
pub(crate) const EXTRA_PET_OFFSET_X: f64 = 168.0;
pub(crate) const EXTRA_PET_OFFSET_Y: f64 = 92.0;

pub(crate) struct DesktopPetState {
    pub(crate) instances: Vec<DesktopPetInstance>,
    pub(crate) next_extra_id: u64,
}

pub(crate) struct DesktopPetInstance {
    pub(crate) id: PetInstanceId,
    pub(crate) window: Option<Window<tauri::Wry>>,
    pub(crate) worker: Option<PetWorker>,
    pub(crate) stop_signal: Option<Arc<AtomicBool>>,
    pub(crate) interaction_signal: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PetInstanceId {
    Primary,
    Extra(u64),
}

impl PetInstanceId {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Primary => PET_WINDOW_LABEL.to_string(),
            Self::Extra(id) => format!("{EXTRA_PET_WINDOW_LABEL_PREFIX}-{id}"),
        }
    }

    pub(crate) fn log_name(self) -> String {
        match self {
            Self::Primary => "primary".to_string(),
            Self::Extra(id) => format!("extra-{id}"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct PetWorker {
    pub(crate) action: Arc<Mutex<PetAction>>,
}

pub(crate) struct PetWorkerContext {
    pub(crate) app: AppHandle,
    pub(crate) window: Window,
    pub(crate) action_state: Arc<Mutex<PetAction>>,
    pub(crate) bubble_state: Arc<Mutex<Option<PetBubble>>>,
    pub(crate) growth_float_state: Arc<Mutex<Option<PetGrowthFloat>>>,
    pub(crate) stop_signal: Arc<AtomicBool>,
    pub(crate) interaction_signal: Arc<AtomicU64>,
    pub(crate) frames: PetAnimationFrames,
}

#[derive(Debug, Clone)]
pub(crate) struct PetBubble {
    pub(crate) text: String,
    pub(crate) expires_at: SystemTime,
}

#[derive(Debug, Clone)]
pub(crate) struct PetGrowthFloat {
    pub(crate) value: i64,
    pub(crate) started_at: SystemTime,
    pub(crate) expires_at: SystemTime,
}

pub(crate) struct PetAnimationFrames {
    pub(crate) crush: Vec<PathBuf>,
    pub(crate) defecate: Vec<PathBuf>,
    pub(crate) drive: Vec<PathBuf>,
    pub(crate) eat: Vec<PathBuf>,
    pub(crate) pants: Vec<PathBuf>,
    pub(crate) read: Vec<PathBuf>,
    pub(crate) rope: Vec<PathBuf>,
    pub(crate) run: Vec<PathBuf>,
    pub(crate) slack: Vec<PathBuf>,
    pub(crate) sleep: Vec<PathBuf>,
    pub(crate) snow: Vec<PathBuf>,
    pub(crate) toy: Vec<PathBuf>,
    pub(crate) work: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PetAction {
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
pub(crate) enum PetBubbleTheme {
    Light,
    Dark,
}

impl PetBubbleTheme {
    pub(crate) fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

impl PetAction {
    pub(crate) fn as_str(self) -> &'static str {
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
pub(crate) struct PetVisualState {
    pub(crate) action: PetAction,
    pub(crate) always_on_top: bool,
    pub(crate) last_window_x: Option<f64>,
    pub(crate) last_window_y: Option<f64>,
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

pub(crate) fn show_desktop_pet_inner(app: &AppHandle) -> LocalResult<bool> {
    let settings = local_db::get_pet_settings(app)?;
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

fn show_desktop_pet_instance(
    app: &AppHandle,
    instance_id: PetInstanceId,
    persist_position: bool,
) -> LocalResult<()> {
    let settings = local_db::get_pet_settings(app)?;
    if !settings.desktop_enabled {
        hide_desktop_pet_inner(app)?;
        return Ok(());
    }
    eprintln!(
        "[desktop-pet] show instance begin instance={} persist_position={persist_position}",
        instance_id.log_name()
    );

    let visual_state = resolve_visual_state(app, &settings)?;
    let (existing_window, interaction_signal) = {
        let state = app.state::<Mutex<DesktopPetState>>();
        let mut guard = state
            .lock()
            .map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
        let instance = get_or_create_instance(&mut guard, instance_id);
        (instance.window.take(), instance.interaction_signal.clone())
    };

    let window = if let Some(window) = existing_window {
        eprintln!(
            "[desktop-pet] reuse existing window instance={}",
            instance_id.log_name()
        );
        configure_pet_window(app, &window, &visual_state, instance_id, persist_position)?;
        window.show().map_err(|err| err.to_string())?;
        window
    } else {
        eprintln!(
            "[desktop-pet] create new window instance={}",
            instance_id.log_name()
        );
        let window = create_pet_window(
            app,
            &visual_state,
            instance_id,
            &interaction_signal,
            persist_position,
        )?;
        if persist_position {
            attach_window_position_persistence(app, &window);
        }
        window
    };

    eprintln!(
        "[desktop-pet] ensure worker instance={}",
        instance_id.log_name()
    );
    let worker = ensure_worker(app, &window, visual_state.action, instance_id)?;
    worker.set_action(visual_state.action);

    let state = app.state::<Mutex<DesktopPetState>>();
    let mut guard = state
        .lock()
        .map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
    let instance = get_or_create_instance(&mut guard, instance_id);
    instance.window = Some(window);
    instance.worker = Some(worker);
    eprintln!(
        "[desktop-pet] show instance completed instance={}",
        instance_id.log_name()
    );
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
        let mut guard = state
            .lock()
            .map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
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

pub(crate) fn hide_desktop_pet_inner(app: &AppHandle) -> LocalResult<bool> {
    let state = app.state::<Mutex<DesktopPetState>>();
    let instances = {
        let mut guard = state
            .lock()
            .map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
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
                persist_pet_window_position(app, &window).ok();
            }
            window.close().map_err(|err| err.to_string())?;
        }
    }

    Ok(false)
}

#[tauri::command]
pub fn get_desktop_pet_status(app: AppHandle) -> LocalResult<DesktopPetStatus> {
    let state = app.state::<Mutex<DesktopPetState>>();
    let guard = state
        .lock()
        .map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
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
    let guard = state
        .lock()
        .map_err(|_| "桌面宠物状态锁已损坏。".to_string())?;
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

mod behavior;
#[cfg(target_os = "macos")]
mod macos_pet_renderer;
#[cfg(windows)]
mod windows_pet_renderer;
mod worker;
