use crate::{
    desktop_pet,
    domain::platform::{self, SupportedPlatform, SUPPORTED_PLATFORMS},
    infrastructure::migrations::CURRENT_SCHEMA_VERSION,
    local_db::{self, LocalResult},
    local_runtime,
};
use serde::Serialize;
use std::{fs, path::Path};
use tauri::AppHandle;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityBaselineStatus {
    pub csp_enabled: bool,
    pub scoped_capabilities: bool,
    pub credential_store_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub app_version: String,
    pub current_platform: Option<SupportedPlatform>,
    pub supported_platforms: Vec<SupportedPlatform>,
    pub database_path: Option<String>,
    pub schema_version: i64,
    pub runtime_status: String,
    pub desktop_pet_diagnostic_log_path: Option<String>,
    pub desktop_pet_diagnostic_log_tail: String,
    pub security_baseline: SecurityBaselineStatus,
}

#[tauri::command]
pub fn get_diagnostics(app: AppHandle) -> LocalResult<DiagnosticsReport> {
    let database_path = local_db::database_path(&app)
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    let runtime_status = local_runtime::detect_runtime_state_for_diagnostics(&app)
        .map(|state| state.status)
        .unwrap_or_else(|error| format!("unknown: {error}"));
    let desktop_pet_diagnostic_log_path = desktop_pet::diagnostic_log_path(&app)
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    let desktop_pet_diagnostic_log_tail =
        desktop_pet::diagnostic_log_tail(&app, 80).unwrap_or_default();

    Ok(DiagnosticsReport {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        current_platform: platform::current_platform(),
        supported_platforms: SUPPORTED_PLATFORMS.to_vec(),
        database_path,
        schema_version: CURRENT_SCHEMA_VERSION,
        runtime_status,
        desktop_pet_diagnostic_log_path,
        desktop_pet_diagnostic_log_tail,
        security_baseline: SecurityBaselineStatus {
            csp_enabled: option_env!("LIBERTY_CSP_DISABLED").is_none(),
            scoped_capabilities: true,
            credential_store_required: true,
        },
    })
}

#[tauri::command]
pub fn export_desktop_pet_diagnostic_log(app: AppHandle, file_path: String) -> LocalResult<()> {
    let trimmed_path = file_path.trim();
    if trimmed_path.is_empty() {
        return Err("导出路径不能为空。".into());
    }

    let source_path = desktop_pet::diagnostic_log_path(&app)?;
    if !source_path.is_file() {
        return Err("桌宠诊断日志尚未生成。".into());
    }

    let target_path = Path::new(trimmed_path);
    if let Some(parent) = target_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    fs::copy(&source_path, target_path).map_err(|err| err.to_string())?;
    Ok(())
}
