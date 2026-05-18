use chrono::Utc;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

use crate::local_db::LocalResult;

pub fn runtime_platform_root(app: &AppHandle, platform_id: &str) -> LocalResult<PathBuf> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| err.to_string())?
        .join("runtime")
        .join(platform_id);
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    Ok(root)
}

pub fn runtime_log_path(app: &AppHandle, platform_id: &str) -> LocalResult<PathBuf> {
    Ok(runtime_platform_root(app, platform_id)?.join("install.log"))
}

pub fn append_install_log(log_path: &Path, bytes: &[u8]) -> LocalResult<()> {
    if bytes.is_empty() {
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|err| err.to_string())?;
    file.write_all(bytes).map_err(|err| err.to_string())
}

pub fn append_install_log_line(log_path: &Path, line: &str) -> LocalResult<()> {
    let prefix = runtime_log_prefix();
    append_install_log(log_path, format!("{prefix} {line}\n").as_bytes())
}

fn runtime_log_prefix() -> String {
    format!("[Liberty-下载进度-{}]", format_display_timestamp())
}

fn format_display_timestamp() -> String {
    Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

pub fn unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
