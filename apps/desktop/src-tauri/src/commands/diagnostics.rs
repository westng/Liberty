use crate::{
    domain::platform::{self, SupportedPlatform, SUPPORTED_PLATFORMS},
    infrastructure::migrations::CURRENT_SCHEMA_VERSION,
    local_db::{self, LocalResult},
    local_runtime,
};
use serde::Serialize;
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

    Ok(DiagnosticsReport {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        current_platform: platform::current_platform(),
        supported_platforms: SUPPORTED_PLATFORMS.to_vec(),
        database_path,
        schema_version: CURRENT_SCHEMA_VERSION,
        runtime_status,
        security_baseline: SecurityBaselineStatus {
            csp_enabled: option_env!("LIBERTY_CSP_DISABLED").is_none(),
            scoped_capabilities: true,
            credential_store_required: true,
        },
    })
}
