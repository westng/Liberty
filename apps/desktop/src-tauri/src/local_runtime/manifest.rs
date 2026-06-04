use serde::Deserialize;

use crate::domain::platform;
use crate::local_db::LocalResult;

const RUNTIME_MANIFEST_JSON: &str = include_str!("../../resources/runtime-manifest.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifest {
    pub runtime_version: String,
    pub python_version: String,
    pub platforms: Vec<PlatformRuntime>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlatformRuntime {
    pub platform_id: String,
    pub asr_backend: Option<String>,
    pub unsupported_reason: Option<String>,
    pub python_bundle: Option<BundledAsset>,
    #[serde(default)]
    pub python_executable_candidates: Vec<String>,
    pub ffmpeg_bundle: Option<BundledAsset>,
    #[serde(default)]
    pub ffmpeg_executable_candidates: Vec<String>,
    pub models_bundle: Option<BundledAsset>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BundledAsset {
    pub file_name: String,
    pub download_url: String,
    #[serde(default)]
    pub sha256: String,
}

pub fn load_manifest() -> LocalResult<RuntimeManifest> {
    serde_json::from_str(RUNTIME_MANIFEST_JSON).map_err(|err| err.to_string())
}

pub fn current_platform_manifest(manifest: &RuntimeManifest) -> LocalResult<PlatformRuntime> {
    let platform_id = current_platform_id()?;
    manifest
        .platforms
        .iter()
        .find(|item| item.platform_id == platform_id)
        .cloned()
        .ok_or_else(|| format!("暂不支持当前平台：{platform_id}"))
}

pub fn current_platform_id() -> LocalResult<&'static str> {
    platform::current_platform_id()
}
