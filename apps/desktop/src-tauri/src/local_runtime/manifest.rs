use serde::{de::Error as _, Deserialize, Deserializer};
use std::collections::HashMap;

use crate::domain::platform;
use crate::local_db::LocalResult;

const RUNTIME_MANIFEST_JSON: &str = include_str!("../../resources/runtime-manifest.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifest {
    pub runtime_version: String,
    pub python_version: String,
    pub model_set_version: String,
    pub model_profile: String,
    #[serde(default)]
    pub download_sources: Vec<RuntimeDownloadSource>,
    pub platforms: Vec<PlatformRuntime>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDownloadSource {
    pub source_id: String,
    pub name_zh: String,
    #[serde(default)]
    pub pip_index_url: Option<String>,
    #[serde(default)]
    pub model_endpoint: Option<String>,
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
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BundledAsset {
    pub file_name: String,
    #[serde(default)]
    pub urls: HashMap<String, String>,
    #[serde(deserialize_with = "deserialize_sha256")]
    pub sha256: String,
}

pub fn load_manifest() -> LocalResult<RuntimeManifest> {
    parse_manifest(RUNTIME_MANIFEST_JSON)
}

pub fn expected_asset_sha256(file_name: &str, download_url: &str) -> LocalResult<String> {
    let manifest = load_manifest()?;
    let expected = manifest
        .platforms
        .iter()
        .flat_map(|platform| [&platform.python_bundle, &platform.ffmpeg_bundle])
        .flatten()
        .find(|asset| {
            asset.file_name == file_name && asset.urls.values().any(|url| url == download_url)
        })
        .map(|asset| asset.sha256.clone())
        .ok_or_else(|| {
            format!("运行时资源未在受信任 manifest 中登记：{file_name} ({download_url})")
        });
    expected
}

fn parse_manifest(source: &str) -> LocalResult<RuntimeManifest> {
    let manifest: RuntimeManifest = serde_json::from_str(source).map_err(|err| err.to_string())?;
    for platform in &manifest.platforms {
        let supported = platform
            .unsupported_reason
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
            .is_none();
        if supported && platform.python_bundle.is_none() {
            return Err(format!(
                "{} 缺少托管 Python 运行时资源。",
                platform.platform_id
            ));
        }
        if supported && platform.ffmpeg_bundle.is_none() {
            return Err(format!(
                "{} 缺少托管 FFmpeg 运行时资源。",
                platform.platform_id
            ));
        }
        for asset in [&platform.python_bundle, &platform.ffmpeg_bundle]
            .into_iter()
            .flatten()
        {
            if asset.file_name.trim().is_empty() || asset.urls.is_empty() {
                return Err(format!(
                    "{} 的运行时资源必须同时提供文件名和下载地址。",
                    platform.platform_id
                ));
            }
            for url in asset.urls.values() {
                validate_immutable_asset_url(url).map_err(|err| {
                    format!("{} / {}: {err}", platform.platform_id, asset.file_name)
                })?;
            }
        }
    }
    Ok(manifest)
}

fn deserialize_sha256<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(D::Error::custom(
            "sha256 必须是 64 位十六进制字符，不能为空",
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn validate_immutable_asset_url(url: &str) -> LocalResult<()> {
    let normalized = url.trim().to_ascii_lowercase();
    if !normalized.starts_with("https://") {
        return Err("运行时资源必须使用 HTTPS 下载。".into());
    }
    if normalized.contains("/latest/") || normalized.contains("/getrelease/") {
        return Err("运行时资源 URL 必须指向不可变的版本。".into());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{load_manifest, parse_manifest};

    fn manifest_with_asset(asset: &str) -> String {
        format!(
            r#"{{"runtimeVersion":"1","pythonVersion":"1","modelSetVersion":"1","modelProfile":"x","platforms":[{{"platformId":"test","unsupportedReason":"fixture","pythonBundle":{asset},"pythonExecutableCandidates":[],"ffmpegExecutableCandidates":[]}}]}}"#
        )
    }

    #[test]
    fn embedded_manifest_has_valid_checksums_and_immutable_urls() {
        load_manifest().expect("embedded manifest should pass supply-chain validation");
    }

    #[test]
    fn embedded_darwin_platforms_have_managed_ffmpeg_archives() {
        let manifest = load_manifest().expect("embedded manifest should load");
        for platform_id in ["darwin-aarch64", "darwin-x64"] {
            let platform = manifest
                .platforms
                .iter()
                .find(|platform| platform.platform_id == platform_id)
                .expect("Darwin platform should be present");
            let asset = platform
                .ffmpeg_bundle
                .as_ref()
                .expect("supported Darwin platform should provide managed FFmpeg");
            assert!(asset.file_name.ends_with(".zip"));
            assert!(asset.urls.contains_key("official"));
        }
    }

    #[test]
    fn manifest_rejects_supported_platform_without_ffmpeg_bundle() {
        let source = r#"{"runtimeVersion":"1","pythonVersion":"1","modelSetVersion":"1","modelProfile":"x","platforms":[{"platformId":"darwin-aarch64","pythonBundle":{"fileName":"python.zip","urls":{"official":"https://example.com/v1/python.zip"},"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"pythonExecutableCandidates":[],"ffmpegExecutableCandidates":[]}]}"#;
        let error = parse_manifest(source).expect_err("managed FFmpeg should be required");
        assert!(error.contains("缺少托管 FFmpeg"));
    }

    #[test]
    fn manifest_rejects_missing_empty_and_malformed_sha256() {
        let base =
            r#"{"fileName":"asset.zip","urls":{"official":"https://example.com/v1/asset.zip"}"#;
        assert!(parse_manifest(&manifest_with_asset(&format!("{base}}}"))).is_err());
        assert!(
            parse_manifest(&manifest_with_asset(&format!("{base},\"sha256\":\"\"}}"))).is_err()
        );
        assert!(parse_manifest(&manifest_with_asset(&format!(
            "{base},\"sha256\":\"abc\"}}"
        )))
        .is_err());
    }

    #[test]
    fn manifest_rejects_mutable_asset_urls() {
        let asset = format!(
            r#"{{"fileName":"asset.zip","urls":{{"official":"https://example.com/latest/asset.zip"}},"sha256":"{}"}}"#,
            "a".repeat(64)
        );
        assert!(parse_manifest(&manifest_with_asset(&asset)).is_err());
    }
}
