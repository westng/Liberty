use crate::local_db::{self, AppSettings, LocalResult};
use tauri::AppHandle;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> LocalResult<AppSettings> {
    local_db::get_settings(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, mut settings: AppSettings) -> LocalResult<()> {
    let stored = local_db::get_settings(&app)?;
    settings.python_path = stored.python_path;
    settings.ffmpeg_path = stored.ffmpeg_path;
    settings.python_runtime_source = stored.python_runtime_source;
    settings.ffmpeg_runtime_source = stored.ffmpeg_runtime_source;
    local_db::save_settings(&app, &settings)
}
