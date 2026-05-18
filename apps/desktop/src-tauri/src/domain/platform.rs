use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PlatformValidationLevel {
    Primary,
    Extended,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportedPlatform {
    pub id: &'static str,
    pub label: &'static str,
    pub rust_target: &'static str,
    pub validation_level: PlatformValidationLevel,
}

pub const SUPPORTED_PLATFORMS: [SupportedPlatform; 4] = [
    SupportedPlatform {
        id: "darwin-aarch64",
        label: "macOS Apple Silicon",
        rust_target: "aarch64-apple-darwin",
        validation_level: PlatformValidationLevel::Primary,
    },
    SupportedPlatform {
        id: "darwin-x64",
        label: "macOS Intel",
        rust_target: "x86_64-apple-darwin",
        validation_level: PlatformValidationLevel::Primary,
    },
    SupportedPlatform {
        id: "windows-x64",
        label: "Windows x64",
        rust_target: "x86_64-pc-windows-msvc",
        validation_level: PlatformValidationLevel::Primary,
    },
    SupportedPlatform {
        id: "windows-x86",
        label: "Windows x86",
        rust_target: "i686-pc-windows-msvc",
        validation_level: PlatformValidationLevel::Extended,
    },
];

pub fn current_platform() -> Option<SupportedPlatform> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        platform_by_id("darwin-aarch64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        platform_by_id("darwin-x64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        platform_by_id("windows-x64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86")) {
        platform_by_id("windows-x86")
    } else {
        None
    }
}

pub fn current_platform_id() -> Result<&'static str, String> {
    current_platform()
        .map(|platform| platform.id)
        .ok_or_else(|| "当前平台暂不支持托管本地运行环境。".to_string())
}

pub fn platform_by_id(id: &str) -> Option<SupportedPlatform> {
    SUPPORTED_PLATFORMS
        .iter()
        .copied()
        .find(|platform| platform.id == id)
}
