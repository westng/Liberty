use crate::local_db::{self, AppSettings, AppSettingsSnapshot, LocalResult, UiPreferences};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, WebviewWindow};

const SETTINGS_CONFLICT: &str = "settings_conflict";
const SETTINGS_SAVE_FAILED: &str = "settings_save_failed";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicSettingsSnapshot {
    theme_mode: String,
    liquid_glass_style: String,
    accent_color: String,
    locale: String,
    backend_url: String,
    api_token_configured: bool,
    processing_mode: String,
    default_hotwords: String,
    summary_template: String,
    concurrency: u32,
    python_path: String,
    ffmpeg_path: String,
    python_runtime_source: String,
    ffmpeg_runtime_source: String,
    runner_script_path: String,
    local_asr_device: String,
    local_asr_threads: u32,
    local_asr_batch_size_seconds: u32,
    runtime_download_source: String,
    #[serde(default)]
    settings_revision: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsInput {
    #[serde(flatten)]
    settings: PublicSettingsSnapshot,
    #[serde(default)]
    credential: SettingsCredentialUpdate,
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum SettingsCredentialUpdate {
    #[default]
    Keep,
    Set {
        value: String,
    },
    Clear,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCommandError {
    code: &'static str,
    message: String,
    current: Option<Box<PublicSettingsSnapshot>>,
}

#[tauri::command]
pub fn get_settings(app: AppHandle, window: WebviewWindow) -> LocalResult<PublicSettingsSnapshot> {
    require_main_window(window.label())?;
    local_db::get_settings_snapshot(&app).map(public_settings_snapshot)
}

#[tauri::command]
pub fn get_ui_preferences(app: AppHandle) -> LocalResult<UiPreferences> {
    local_db::get_ui_preferences(&app)
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    window: WebviewWindow,
    settings: SaveSettingsInput,
) -> Result<PublicSettingsSnapshot, SettingsCommandError> {
    require_main_window(window.label()).map_err(SettingsCommandError::failed)?;
    let stored = local_db::get_settings_snapshot(&app).map_err(SettingsCommandError::failed)?;
    let prepared =
        prepare_settings_snapshot(settings, &stored).map_err(SettingsCommandError::failed)?;
    local_db::save_settings_snapshot(&app, &prepared)
        .map(public_settings_snapshot)
        .map_err(|message| {
            let current = local_db::get_settings_snapshot(&app)
                .ok()
                .map(public_settings_snapshot);
            SettingsCommandError::from_save_failure(message, prepared.settings_revision, current)
        })
}

fn require_main_window(window_label: &str) -> LocalResult<()> {
    if window_label == "main" {
        Ok(())
    } else {
        Err("当前窗口无权读取或修改完整设置。".into())
    }
}

fn prepare_settings_snapshot(
    incoming: SaveSettingsInput,
    stored: &AppSettingsSnapshot,
) -> LocalResult<AppSettingsSnapshot> {
    let public = incoming.settings;
    let api_token = match incoming.credential {
        SettingsCredentialUpdate::Keep => stored.settings.api_token.clone(),
        SettingsCredentialUpdate::Set { value } => {
            let value = value.trim();
            if value.is_empty() {
                return Err("设置远端 API Token 时凭据不能为空。".into());
            }
            value.to_string()
        }
        SettingsCredentialUpdate::Clear => String::new(),
    };
    let settings = AppSettings {
        theme_mode: public.theme_mode,
        liquid_glass_style: public.liquid_glass_style,
        accent_color: public.accent_color,
        locale: public.locale,
        backend_url: public.backend_url,
        api_token,
        processing_mode: public.processing_mode,
        default_hotwords: public.default_hotwords,
        summary_template: public.summary_template,
        concurrency: public.concurrency,
        python_path: stored.settings.python_path.clone(),
        ffmpeg_path: stored.settings.ffmpeg_path.clone(),
        python_runtime_source: stored.settings.python_runtime_source.clone(),
        ffmpeg_runtime_source: stored.settings.ffmpeg_runtime_source.clone(),
        runner_script_path: public.runner_script_path,
        local_asr_device: public.local_asr_device,
        local_asr_threads: public.local_asr_threads,
        local_asr_batch_size_seconds: public.local_asr_batch_size_seconds,
        runtime_download_source: public.runtime_download_source,
    };
    Ok(AppSettingsSnapshot {
        settings,
        settings_revision: public.settings_revision.or(stored.settings_revision),
    })
}

fn public_settings_snapshot(snapshot: AppSettingsSnapshot) -> PublicSettingsSnapshot {
    let settings = snapshot.settings;
    PublicSettingsSnapshot {
        theme_mode: settings.theme_mode,
        liquid_glass_style: settings.liquid_glass_style,
        accent_color: settings.accent_color,
        locale: settings.locale,
        backend_url: settings.backend_url,
        api_token_configured: !settings.api_token.trim().is_empty(),
        processing_mode: settings.processing_mode,
        default_hotwords: settings.default_hotwords,
        summary_template: settings.summary_template,
        concurrency: settings.concurrency,
        python_path: settings.python_path,
        ffmpeg_path: settings.ffmpeg_path,
        python_runtime_source: settings.python_runtime_source,
        ffmpeg_runtime_source: settings.ffmpeg_runtime_source,
        runner_script_path: settings.runner_script_path,
        local_asr_device: settings.local_asr_device,
        local_asr_threads: settings.local_asr_threads,
        local_asr_batch_size_seconds: settings.local_asr_batch_size_seconds,
        runtime_download_source: settings.runtime_download_source,
        settings_revision: snapshot.settings_revision,
    }
}

impl SettingsCommandError {
    fn failed(message: String) -> Self {
        Self {
            code: SETTINGS_SAVE_FAILED,
            message,
            current: None,
        }
    }

    fn from_save_failure(
        message: String,
        expected_revision: Option<i64>,
        current: Option<PublicSettingsSnapshot>,
    ) -> Self {
        let conflicted = expected_revision.is_some()
            && current
                .as_ref()
                .is_some_and(|snapshot| snapshot.settings_revision != expected_revision);
        Self {
            code: if conflicted {
                SETTINGS_CONFLICT
            } else {
                SETTINGS_SAVE_FAILED
            },
            message,
            current: current.map(Box::new),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        prepare_settings_snapshot, public_settings_snapshot, require_main_window,
        PublicSettingsSnapshot, SaveSettingsInput, SettingsCommandError, SettingsCredentialUpdate,
        SETTINGS_CONFLICT,
    };
    use crate::local_db::{AppSettings, AppSettingsSnapshot, UiPreferences};

    fn snapshot(revision: Option<i64>) -> AppSettingsSnapshot {
        AppSettingsSnapshot {
            settings: AppSettings::default(),
            settings_revision: revision,
        }
    }

    fn public_snapshot(revision: Option<i64>) -> PublicSettingsSnapshot {
        public_settings_snapshot(snapshot(revision))
    }

    fn save_input(
        revision: Option<i64>,
        credential: SettingsCredentialUpdate,
    ) -> SaveSettingsInput {
        SaveSettingsInput {
            settings: public_snapshot(revision),
            credential,
        }
    }

    #[test]
    fn keep_preserves_credentials_and_runtime_fields() {
        let mut stored = snapshot(Some(7));
        stored.settings.api_token = "stored-secret".into();
        stored.settings.python_path = "/managed/python".into();
        stored.settings.ffmpeg_path = "/managed/ffmpeg".into();
        stored.settings.python_runtime_source = "system".into();
        stored.settings.ffmpeg_runtime_source = "system".into();

        let prepared =
            prepare_settings_snapshot(save_input(None, SettingsCredentialUpdate::Keep), &stored)
                .expect("prepare settings");

        assert_eq!(prepared.settings_revision, Some(7));
        assert_eq!(prepared.settings.api_token, "stored-secret");
        assert_eq!(prepared.settings.python_path, "/managed/python");
        assert_eq!(prepared.settings.ffmpeg_path, "/managed/ffmpeg");
        assert_eq!(prepared.settings.python_runtime_source, "system");
        assert_eq!(prepared.settings.ffmpeg_runtime_source, "system");
    }

    #[test]
    fn credential_updates_are_explicit() {
        let mut stored = snapshot(Some(2));
        stored.settings.api_token = "stored-secret".into();

        let replaced = prepare_settings_snapshot(
            save_input(
                Some(2),
                SettingsCredentialUpdate::Set {
                    value: " new-secret ".into(),
                },
            ),
            &stored,
        )
        .expect("replace token");
        assert_eq!(replaced.settings.api_token, "new-secret");

        let cleared = prepare_settings_snapshot(
            save_input(Some(2), SettingsCredentialUpdate::Clear),
            &stored,
        )
        .expect("clear token");
        assert!(cleared.settings.api_token.is_empty());
    }

    #[test]
    fn public_settings_and_conflicts_exclude_secret_values() {
        let mut internal = snapshot(Some(4));
        internal.settings.api_token = "never-serialize-me".into();
        let current = public_settings_snapshot(internal);
        let serialized = serde_json::to_value(&current).expect("serialize public settings");
        assert_eq!(
            serialized.get("apiTokenConfigured"),
            Some(&serde_json::json!(true))
        );
        assert!(serialized.get("apiToken").is_none());

        let error = SettingsCommandError::from_save_failure(
            "stale revision".into(),
            Some(3),
            Some(current),
        );
        assert_eq!(error.code, SETTINGS_CONFLICT);
        let serialized_error = serde_json::to_value(error).expect("serialize error");
        assert!(serialized_error
            .to_string()
            .find("never-serialize-me")
            .is_none());
    }

    #[test]
    fn full_settings_are_restricted_to_main_window() {
        assert_eq!(require_main_window("main"), Ok(()));
        assert!(require_main_window("meeting-notes").is_err());
        assert!(require_main_window("model-editor").is_err());
    }

    #[test]
    fn ui_preferences_serialization_excludes_sensitive_fields() {
        let value = serde_json::to_value(UiPreferences {
            theme_mode: "dark".into(),
            liquid_glass_style: "tinted".into(),
            accent_color: "#123456".into(),
            locale: "en-US".into(),
        })
        .expect("serialize preferences");

        assert_eq!(value.as_object().map(serde_json::Map::len), Some(4));
        assert!(value.get("backendUrl").is_none());
        assert!(value.get("apiTokenConfigured").is_none());
        assert!(value.get("pythonPath").is_none());
    }
}
