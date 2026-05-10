use crate::local_db::{self, AppSettings, LocalResult};
use tauri::AppHandle;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> LocalResult<AppSettings> {
    local_db::get_settings(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> LocalResult<()> {
    local_db::save_settings(&app, &settings)
}
