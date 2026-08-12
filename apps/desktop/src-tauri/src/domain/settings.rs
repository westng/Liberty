use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub theme_mode: String,
    pub liquid_glass_style: String,
    pub accent_color: String,
    pub locale: String,
    pub backend_url: String,
    pub api_token: String,
    pub processing_mode: String,
    pub default_hotwords: String,
    pub summary_template: String,
    pub concurrency: u32,
    pub python_path: String,
    pub ffmpeg_path: String,
    pub python_runtime_source: String,
    pub ffmpeg_runtime_source: String,
    pub runner_script_path: String,
    pub local_asr_device: String,
    pub local_asr_threads: u32,
    pub local_asr_batch_size_seconds: u32,
    pub runtime_download_source: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsSnapshot {
    #[serde(flatten)]
    pub settings: AppSettings,
    #[serde(default)]
    pub settings_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialUpdate {
    Keep,
    Set(String),
    Clear,
}

pub fn prepare_settings_snapshot(
    mut incoming: AppSettingsSnapshot,
    credential: CredentialUpdate,
    stored: &AppSettingsSnapshot,
) -> Result<AppSettingsSnapshot, String> {
    incoming.settings.api_token = match credential {
        CredentialUpdate::Keep => stored.settings.api_token.clone(),
        CredentialUpdate::Set(value) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("设置远端 API Token 时凭据不能为空。".into());
            }
            value.to_string()
        }
        CredentialUpdate::Clear => String::new(),
    };
    incoming.settings.python_path = stored.settings.python_path.clone();
    incoming.settings.ffmpeg_path = stored.settings.ffmpeg_path.clone();
    incoming.settings.python_runtime_source = stored.settings.python_runtime_source.clone();
    incoming.settings.ffmpeg_runtime_source = stored.settings.ffmpeg_runtime_source.clone();
    incoming.settings_revision = incoming.settings_revision.or(stored.settings_revision);
    Ok(incoming)
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: "auto".into(),
            liquid_glass_style: "transparent".into(),
            accent_color: "#2f6dff".into(),
            locale: "zh-CN".into(),
            backend_url: String::new(),
            api_token: String::new(),
            processing_mode: "local".into(),
            default_hotwords: "SeACo-Paraformer, FunASR, 会议纪要".into(),
            summary_template: "表格版会议纪要".into(),
            concurrency: 2,
            python_path: String::new(),
            ffmpeg_path: String::new(),
            python_runtime_source: "managed".into(),
            ffmpeg_runtime_source: "managed".into(),
            runner_script_path: String::new(),
            local_asr_device: "auto".into(),
            local_asr_threads: 0,
            local_asr_batch_size_seconds: 300,
            runtime_download_source: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_save_preserves_private_runtime_state() {
        let mut stored = AppSettingsSnapshot {
            settings: AppSettings::default(),
            settings_revision: Some(7),
        };
        stored.settings.api_token = "stored-secret".into();
        stored.settings.python_path = "/managed/python".into();
        stored.settings.python_runtime_source = "system".into();
        let incoming = AppSettingsSnapshot {
            settings: AppSettings::default(),
            settings_revision: None,
        };

        let prepared = prepare_settings_snapshot(incoming, CredentialUpdate::Keep, &stored)
            .expect("prepare settings");

        assert_eq!(prepared.settings_revision, Some(7));
        assert_eq!(prepared.settings.api_token, "stored-secret");
        assert_eq!(prepared.settings.python_path, "/managed/python");
        assert_eq!(prepared.settings.python_runtime_source, "system");
    }
}
