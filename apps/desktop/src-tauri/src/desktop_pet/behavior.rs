use crate::local_db::{self, LocalResult, PetEventLedgerEntry, PetSettings};
use chrono::Utc;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, LogicalPosition, Manager, Window, WindowEvent};

use super::{
    PetAction, PetAnimationFrames, PetBubble, PetGrowthFloat, PetInstanceId, PetVisualState,
    BUBBLE_VISIBLE_MS, DAILY_ACTION_BUCKET_MS, EXTRA_PET_OFFSET_X, EXTRA_PET_OFFSET_Y,
    NEEDY_AFTER_MS, PET_WINDOW_HEIGHT, PET_WINDOW_WIDTH, RECENT_EVENT_ACTION_HOLD_MS,
};

pub(crate) fn handle_desktop_pet_interaction(
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

pub(crate) fn current_bubble_text(bubble_state: &Arc<Mutex<Option<PetBubble>>>) -> Option<String> {
    let mut guard = bubble_state.lock().ok()?;
    let bubble = guard.as_ref()?;
    if SystemTime::now() >= bubble.expires_at {
        *guard = None;
        return None;
    }
    Some(bubble.text.clone())
}

pub(crate) fn current_growth_float(
    growth_float_state: &Arc<Mutex<Option<PetGrowthFloat>>>,
) -> Option<PetGrowthFloat> {
    let mut guard = growth_float_state.lock().ok()?;
    let growth_float = guard.as_ref()?;
    if SystemTime::now() >= growth_float.expires_at {
        *guard = None;
        return None;
    }
    Some(growth_float.clone())
}

pub(crate) fn speech_line_from_event(event: &PetEventLedgerEntry) -> Option<String> {
    let metadata = event.metadata.as_deref()?;
    if !matches!(
        event.event_type.as_str(),
        "store_food"
            | "store_equip"
            | "blind_box_reward"
            | "blind_box_empty"
            | "blind_box_duplicate"
    ) {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(metadata) {
        return value
            .get("zh")
            .and_then(|line| line.as_str())
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string);
    }

    metadata
        .rsplit_once('|')
        .map(|(_, line)| line.trim())
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
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

pub(crate) fn should_show_proactive_bubble(
    settings: &PetSettings,
    last_proactive_bubble: SystemTime,
) -> bool {
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

pub(crate) fn select_proactive_dialogue(app: &AppHandle) -> String {
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
        "你继续推进，我在桌面上给你盯着状态。".to_string(),
    ];
    let index = (now_ms() as usize / 7) % lines.len();
    lines[index].clone()
}

pub(crate) fn resolve_visual_state(
    app: &AppHandle,
    settings: &PetSettings,
) -> LocalResult<PetVisualState> {
    let profile = local_db::get_pet_profile(app)?;
    let jobs = local_db::list_jobs(app)?;
    let latest_event = local_db::list_pet_event_ledger(app, 1)?.into_iter().next();
    let environment_state = jobs
        .iter()
        .map(|job| job.overall_status.as_str())
        .find(|status| {
            matches!(
                *status,
                "transcribing" | "speaker_processing" | "summarizing" | "queued"
            )
        });

    Ok(PetVisualState {
        action: resolve_pet_action(
            environment_state,
            latest_event.as_ref(),
            &profile.current_mood,
        ),
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
        "uploaded" => PetAction::Reading,
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
        "bored" => PetAction::Reading,
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
        "store_food" => PetAction::Eat,
        "store_equip" => PetAction::Snow,
        "blind_box_reward" => PetAction::Run,
        "blind_box_empty" => PetAction::Toy,
        "blind_box_duplicate" => PetAction::Snow,
        "job_created" => PetAction::Drive,
        "transcription_started" => PetAction::Work,
        "transcription_completed" | "ai_summary_completed" | "export_completed" => PetAction::Snow,
        "daily_open" => PetAction::Slack,
        _ => get_action_for_mood(mood),
    }
}

fn get_daily_idle_action(latest_event_at: u64, idle_ms: u64) -> PetAction {
    const ACTIONS: [PetAction; 15] = [
        PetAction::Slack,
        PetAction::Toy,
        PetAction::Rope,
        PetAction::Drive,
        PetAction::Crush,
        PetAction::Defecate,
        PetAction::Eat,
        PetAction::Gaming,
        PetAction::Pants,
        PetAction::Reading,
        PetAction::Run,
        PetAction::Sleep,
        PetAction::Snow,
        PetAction::Studying,
        PetAction::Work,
    ];
    let bucket = ((latest_event_at + idle_ms) / DAILY_ACTION_BUCKET_MS) as usize;
    ACTIONS[bucket % ACTIONS.len()]
}

impl PetAnimationFrames {
    pub(crate) fn frame_path(
        &self,
        action: PetAction,
        frame_index: usize,
    ) -> LocalResult<&PathBuf> {
        let frames = match action {
            PetAction::Crush => &self.crush,
            PetAction::Defecate => &self.defecate,
            PetAction::Drive => &self.drive,
            PetAction::Eat => &self.eat,
            PetAction::Gaming => &self.gaming,
            PetAction::Pants => &self.pants,
            PetAction::Reading => &self.reading,
            PetAction::Rope => &self.rope,
            PetAction::Run => &self.run,
            PetAction::Slack => &self.slack,
            PetAction::Sleep => &self.sleep,
            PetAction::Snow => &self.snow,
            PetAction::Studying => &self.studying,
            PetAction::Toy => &self.toy,
            PetAction::Work => &self.work,
        };
        if frames.is_empty() {
            return Err(format!("宠物资源组为空：{}", action.as_str()));
        }
        Ok(&frames[frame_index % frames.len()])
    }
}

pub(crate) fn load_pet_animation_frames(app: &AppHandle) -> LocalResult<PetAnimationFrames> {
    Ok(PetAnimationFrames {
        crush: resolve_pet_frame_paths(app, PetAction::Crush)?,
        defecate: resolve_pet_frame_paths(app, PetAction::Defecate)?,
        drive: resolve_pet_frame_paths(app, PetAction::Drive)?,
        eat: resolve_pet_frame_paths(app, PetAction::Eat)?,
        gaming: resolve_pet_frame_paths(app, PetAction::Gaming)?,
        pants: resolve_pet_frame_paths(app, PetAction::Pants)?,
        reading: resolve_pet_frame_paths(app, PetAction::Reading)?,
        rope: resolve_pet_frame_paths(app, PetAction::Rope)?,
        run: resolve_pet_frame_paths(app, PetAction::Run)?,
        slack: resolve_pet_frame_paths(app, PetAction::Slack)?,
        sleep: resolve_pet_frame_paths(app, PetAction::Sleep)?,
        snow: resolve_pet_frame_paths(app, PetAction::Snow)?,
        studying: resolve_pet_frame_paths(app, PetAction::Studying)?,
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
        .filter(|path| pet_frame_number(path).is_some())
        .collect::<Vec<_>>();

    entries.sort_by_key(|path| pet_frame_number(path).unwrap_or(u32::MAX));
    Ok(entries)
}

fn pet_frame_number(path: &Path) -> Option<u32> {
    if path.file_name()?.to_str()?.starts_with("._") {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    stem.parse::<u32>()
        .ok()
        .or_else(|| stem.rsplit_once('_')?.1.parse::<u32>().ok())
}

fn pet_resource_root(app: &AppHandle) -> LocalResult<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest_candidates = [
        manifest_dir.join("../src/assets/images/action"),
        manifest_dir.join("resources/pet"),
    ];
    for candidate in manifest_candidates {
        if is_pet_resource_root(&candidate) {
            return Ok(candidate);
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        let dev_candidates = [
            current_dir.join("src/assets/images/action"),
            current_dir.join("apps/desktop/src/assets/images/action"),
            current_dir.join("resources/pet"),
            current_dir.join("src-tauri/resources/pet"),
            current_dir.join("apps/desktop/src-tauri/resources/pet"),
        ];
        for candidate in dev_candidates {
            if is_pet_resource_root(&candidate) {
                return Ok(candidate);
            }
        }
    }

    let resource_dir = app.path().resource_dir().map_err(|err| err.to_string())?;
    let packaged = resource_dir.join("pet");
    if packaged.exists() {
        return Ok(packaged);
    }

    Ok(app
        .path()
        .resolve("resources/pet", tauri::path::BaseDirectory::Resource)
        .unwrap_or(resource_dir.join("pet")))
}

fn is_pet_resource_root(path: &Path) -> bool {
    path.join("reading").is_dir()
        && path.join("gaming").is_dir()
        && path.join("studying").is_dir()
        && path.join("slack").join("slack_01.png").is_file()
}

pub(crate) fn resolve_pet_position(
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
        .or_else(|| {
            app.available_monitors()
                .ok()
                .and_then(|monitors| monitors.into_iter().next())
        });

    let Some(monitor) = monitor else {
        return Ok(LogicalPosition::new(80.0, 80.0));
    };

    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    Ok(LogicalPosition::new(
        (work_area.position.x as f64 + work_area.size.width as f64
            - PET_WINDOW_WIDTH * scale
            - 24.0)
            / scale,
        (work_area.position.y as f64 + work_area.size.height as f64
            - PET_WINDOW_HEIGHT * scale
            - 24.0)
            / scale,
    ))
}

fn is_position_visible(app: &AppHandle, x: f64, y: f64) -> LocalResult<bool> {
    let monitors = app.available_monitors().map_err(|err| err.to_string())?;
    Ok(monitors.into_iter().any(|monitor| {
        let scale = monitor.scale_factor();
        let work_area = monitor.work_area();
        let px = x * scale;
        let py = y * scale;
        let max_x =
            work_area.position.x as f64 + work_area.size.width as f64 - PET_WINDOW_WIDTH * scale;
        let max_y =
            work_area.position.y as f64 + work_area.size.height as f64 - PET_WINDOW_HEIGHT * scale;
        px >= work_area.position.x as f64
            && py >= work_area.position.y as f64
            && px <= max_x
            && py <= max_y
    }))
}

pub(crate) fn attach_window_position_persistence(app: &AppHandle, window: &Window) {
    let app = app.clone();
    let window_for_event = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Moved(_)) {
            persist_pet_window_position(&app, &window_for_event).ok();
        }
    });
}

pub(crate) fn persist_pet_window_position(app: &AppHandle, window: &Window) -> LocalResult<()> {
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

#[cfg(test)]
mod tests {
    use super::pet_frame_number;
    use std::path::Path;

    #[test]
    fn ignores_apple_double_metadata_files() {
        assert_eq!(pet_frame_number(Path::new("._pants_03.png")), None);
        assert_eq!(pet_frame_number(Path::new("pants_03.png")), Some(3));
    }
}
