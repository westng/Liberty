use crate::local_db::{
    self, LocalResult, PetCosmeticUnlock, PetEventLedgerEntry, PetProfile, PetSettings,
};
use chrono::Utc;
use serde::Deserialize;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePetSettingsInput {
    pub desktop_enabled: bool,
    pub always_on_top: bool,
    pub muted: bool,
    pub focus_mode_enabled: bool,
    pub proactive_level: i64,
    pub last_window_x: Option<f64>,
    pub last_window_y: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetInteractionInput {
    pub action: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetWorkflowEventInput {
    pub event_type: String,
    #[serde(default)]
    pub metadata: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePetProfileInput {
    pub name: String,
}

#[tauri::command]
pub fn get_pet_profile(app: AppHandle) -> LocalResult<PetProfile> {
    local_db::get_pet_profile(&app)
}

#[tauri::command]
pub fn save_pet_profile(app: AppHandle, input: SavePetProfileInput) -> LocalResult<PetProfile> {
    let normalized_name = input.name.trim();
    if normalized_name.is_empty() {
        return Err("宠物名字不能为空。".into());
    }

    let mut profile = local_db::get_pet_profile(&app)?;
    profile.name = normalized_name.to_string();
    profile.updated_at = Utc::now().to_rfc3339();
    local_db::save_pet_profile(&app, &profile)
}

#[tauri::command]
pub fn get_pet_settings(app: AppHandle) -> LocalResult<PetSettings> {
    local_db::get_pet_settings(&app)
}

#[tauri::command]
pub fn save_pet_settings(app: AppHandle, input: SavePetSettingsInput) -> LocalResult<PetSettings> {
    let settings = PetSettings {
        pet_id: "default-pet".into(),
        desktop_enabled: input.desktop_enabled,
        always_on_top: input.always_on_top,
        muted: input.muted,
        focus_mode_enabled: input.focus_mode_enabled,
        proactive_level: input.proactive_level.clamp(0, 3),
        last_window_x: input.last_window_x,
        last_window_y: input.last_window_y,
        updated_at: Utc::now().to_rfc3339(),
    };

    local_db::save_pet_settings(&app, &settings)
}

#[tauri::command]
pub fn list_pet_event_ledger(
    app: AppHandle,
    limit: Option<usize>,
) -> LocalResult<Vec<PetEventLedgerEntry>> {
    local_db::list_pet_event_ledger(&app, limit.unwrap_or(20).clamp(1, 100))
}

#[tauri::command]
pub fn list_pet_cosmetic_unlocks(app: AppHandle) -> LocalResult<Vec<PetCosmeticUnlock>> {
    local_db::list_pet_cosmetic_unlocks(&app)
}

#[tauri::command]
pub fn apply_pet_interaction(
    app: AppHandle,
    input: PetInteractionInput,
) -> LocalResult<PetProfile> {
    let normalized = input.action.trim().to_lowercase();
    let (event_value, mood) = match normalized.as_str() {
        "pet" => (2, "cheerful"),
        "feed" => (3, "proud"),
        "encourage" => (2, "cheerful"),
        _ => (1, "cheerful"),
    };

    local_db::apply_pet_growth_event(
        &app,
        "interaction",
        normalized.as_str(),
        event_value,
        mood,
        None,
    )
}

#[tauri::command]
pub fn apply_pet_workflow_event(
    app: AppHandle,
    input: PetWorkflowEventInput,
) -> LocalResult<PetProfile> {
    let normalized = input.event_type.trim().to_lowercase();
    let metadata = if input.metadata.trim().is_empty() {
        None
    } else {
        Some(input.metadata.trim())
    };
    let (event_value, mood) = match normalized.as_str() {
        "job_created" => (10, "cheerful"),
        "transcription_started" => (2, "excited"),
        "transcription_completed" => (20, "proud"),
        "ai_summary_completed" => (8, "proud"),
        "export_completed" => (6, "proud"),
        "daily_open" => (1, "idle"),
        _ => (1, "idle"),
    };

    local_db::apply_pet_growth_event(
        &app,
        "workflow",
        normalized.as_str(),
        event_value,
        mood,
        metadata,
    )
}
