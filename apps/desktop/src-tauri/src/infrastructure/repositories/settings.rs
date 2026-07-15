use rusqlite::{params, Connection, OptionalExtension};

use crate::infrastructure::{
    credentials::{credential_key_for_remote_api_token, default_credential_store, CredentialStore},
    ids,
};
use crate::local_db::{AppSettings, LocalResult, UiPreferences};

pub fn load_settings(conn: &Connection) -> LocalResult<AppSettings> {
    load_settings_with_revision(conn).map(|(settings, _)| settings)
}

pub fn load_settings_with_revision(conn: &Connection) -> LocalResult<(AppSettings, i64)> {
    load_settings_with_revision_from_store(conn, &default_credential_store())
}

pub fn load_ui_preferences(conn: &Connection) -> LocalResult<UiPreferences> {
    conn.query_row(
        "SELECT theme_mode, liquid_glass_style, accent_color, locale
         FROM app_settings WHERE id = 1",
        [],
        |row| {
            Ok(UiPreferences {
                theme_mode: row.get(0)?,
                liquid_glass_style: row.get(1)?,
                accent_color: row.get(2)?,
                locale: row.get(3)?,
            })
        },
    )
    .optional()
    .map(|preferences| preferences.unwrap_or_else(default_ui_preferences))
    .map_err(|error| error.to_string())
}

fn default_ui_preferences() -> UiPreferences {
    let settings = AppSettings::default();
    UiPreferences {
        theme_mode: settings.theme_mode,
        liquid_glass_style: settings.liquid_glass_style,
        accent_color: settings.accent_color,
        locale: settings.locale,
    }
}

fn load_settings_with_revision_from_store(
    conn: &Connection,
    credential_store: &dyn CredentialStore,
) -> LocalResult<(AppSettings, i64)> {
    let loaded = conn
        .query_row(
            "SELECT theme_mode, liquid_glass_style, accent_color, locale, backend_url,
                    api_token, COALESCE(api_token_ref, ''), processing_mode,
                    settings_revision,
                    default_hotwords, summary_template, concurrency,
                    python_path, ffmpeg_path, python_runtime_source, ffmpeg_runtime_source,
                    runner_script_path, local_asr_device, local_asr_threads,
                    local_asr_batch_size_seconds, runtime_download_source
             FROM app_settings
             WHERE id = 1",
            [],
            |row| {
                let settings = AppSettings {
                    theme_mode: row.get(0)?,
                    liquid_glass_style: row.get(1)?,
                    accent_color: row.get(2)?,
                    locale: row.get(3)?,
                    backend_url: row.get(4)?,
                    api_token: row.get(5)?,
                    processing_mode: row.get(7)?,
                    default_hotwords: row.get(9)?,
                    summary_template: row.get(10)?,
                    concurrency: row.get::<_, i64>(11)? as u32,
                    python_path: row.get(12)?,
                    ffmpeg_path: row.get(13)?,
                    python_runtime_source: row.get(14)?,
                    ffmpeg_runtime_source: row.get(15)?,
                    runner_script_path: row.get(16)?,
                    local_asr_device: row.get(17)?,
                    local_asr_threads: row.get::<_, i64>(18)? as u32,
                    local_asr_batch_size_seconds: row.get::<_, i64>(19)? as u32,
                    runtime_download_source: row.get(20)?,
                };
                Ok((settings, row.get::<_, String>(6)?, row.get::<_, i64>(8)?))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?;

    match loaded {
        Some((mut settings, credential_reference, revision)) => {
            if !credential_reference.trim().is_empty() {
                settings.api_token = credential_store
                    .get_secret(&credential_reference)
                    .map_err(String::from)?
                    .unwrap_or_default();
            }
            Ok((normalize_settings(settings), revision))
        }
        None => {
            let settings = AppSettings::default();
            save_settings_with_store(
                conn,
                &settings,
                credential_store,
                &next_api_token_reference(0),
            )?;
            Ok((settings, 0))
        }
    }
}

fn save_settings_with_store(
    conn: &Connection,
    settings: &AppSettings,
    credential_store: &dyn CredentialStore,
    staged_reference: &str,
) -> LocalResult<()> {
    let normalized = normalize_settings(settings.clone());
    let exists = conn
        .query_row("SELECT 1 FROM app_settings WHERE id = 1", [], |_| Ok(()))
        .optional()
        .map_err(|err| err.to_string())?
        .is_some();
    if exists {
        return Err(
            "设置已存在；必须携带 settings revision 调用 save_settings_if_revision。".into(),
        );
    }

    let credential = prepare_api_token(
        credential_store,
        "",
        &normalized.api_token,
        staged_reference,
    )?;
    let inserted = match conn.execute(
        "INSERT INTO app_settings (
            id, theme_mode, liquid_glass_style, accent_color, locale, backend_url,
            api_token, api_token_ref, processing_mode, settings_revision,
            default_hotwords, summary_template, concurrency, python_path,
            ffmpeg_path, python_runtime_source, ffmpeg_runtime_source, runner_script_path,
            local_asr_device, local_asr_threads, local_asr_batch_size_seconds,
            runtime_download_source
         ) VALUES (1, ?1, ?2, ?3, ?4, ?5, '', ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
         ON CONFLICT(id) DO NOTHING",
        params![
            normalized.theme_mode,
            normalized.liquid_glass_style,
            normalized.accent_color,
            normalized.locale,
            normalized.backend_url,
            credential.target_reference,
            normalized.processing_mode,
            normalized.default_hotwords,
            normalized.summary_template,
            i64::from(normalized.concurrency),
            normalized.python_path,
            normalized.ffmpeg_path,
            normalized.python_runtime_source,
            normalized.ffmpeg_runtime_source,
            normalized.runner_script_path,
            normalized.local_asr_device,
            i64::from(normalized.local_asr_threads),
            i64::from(normalized.local_asr_batch_size_seconds),
            normalized.runtime_download_source
        ],
    ) {
        Ok(inserted) => inserted,
        Err(error) => {
            return match settings_reference_matches(conn, 0, &credential.target_reference) {
                Ok(true) => {
                    finalize_api_token(credential_store, &credential);
                    Ok(())
                }
                Ok(false) => Err(rollback_api_token(
                    credential_store,
                    &credential,
                    error.to_string(),
                )),
                Err(check_error) => Err(format!(
                    "保存设置失败且无法确认提交结果: {error}; {check_error}"
                )),
            };
        }
    };
    if inserted == 0 {
        return Err(rollback_api_token(
            credential_store,
            &credential,
            "设置已存在；必须携带 settings revision 调用 save_settings_if_revision。".into(),
        ));
    }
    finalize_api_token(credential_store, &credential);
    Ok(())
}

pub fn save_settings_if_revision(
    conn: &Connection,
    settings: &AppSettings,
    expected_revision: i64,
) -> LocalResult<i64> {
    save_settings_if_revision_with_store(
        conn,
        settings,
        expected_revision,
        &default_credential_store(),
        &next_api_token_reference(expected_revision.saturating_add(1)),
    )
}

pub fn set_runtime_component_source(
    conn: &Connection,
    component: &str,
    source: &str,
) -> LocalResult<i64> {
    if !matches!(source, "managed" | "system") {
        return Err("不支持的运行环境来源。".into());
    }
    let statement = match component {
        "python" => {
            "UPDATE app_settings
             SET python_runtime_source = ?1,
                 python_path = CASE WHEN ?1 = 'managed' THEN '' ELSE python_path END,
                 settings_revision = settings_revision + 1
             WHERE id = 1
             RETURNING settings_revision"
        }
        "ffmpeg" => {
            "UPDATE app_settings
             SET ffmpeg_runtime_source = ?1,
                 ffmpeg_path = CASE WHEN ?1 = 'managed' THEN '' ELSE ffmpeg_path END,
                 settings_revision = settings_revision + 1
             WHERE id = 1
             RETURNING settings_revision"
        }
        _ => return Err("不支持的运行环境组件。".into()),
    };
    conn.query_row(statement, params![source], |row| row.get(0))
        .map_err(|error| error.to_string())
}

pub fn publish_detected_runtime_path(
    conn: &Connection,
    platform_id: &str,
    component: &str,
    path: &str,
    expected_source: &str,
    expected_generation: u64,
) -> LocalResult<Option<i64>> {
    let generation = i64::try_from(expected_generation)
        .map_err(|_| "runtime generation 超出 SQLite INTEGER 范围。".to_string())?;
    let statement = match component {
        "python" => {
            "UPDATE app_settings
             SET python_path = ?1, settings_revision = settings_revision + 1
             WHERE id = 1
               AND python_runtime_source = ?2
               AND EXISTS (
                 SELECT 1 FROM runtime_component_state
                 WHERE platform_id = ?3 AND component = 'python' AND source = ?2
                   AND operation_generation = ?4
               )
             RETURNING settings_revision"
        }
        "ffmpeg" => {
            "UPDATE app_settings
             SET ffmpeg_path = ?1, settings_revision = settings_revision + 1
             WHERE id = 1
               AND ffmpeg_runtime_source = ?2
               AND EXISTS (
                 SELECT 1 FROM runtime_component_state
                 WHERE platform_id = ?3 AND component = 'ffmpeg' AND source = ?2
                   AND operation_generation = ?4
               )
             RETURNING settings_revision"
        }
        _ => return Err("不支持的运行环境组件。".into()),
    };
    conn.query_row(
        statement,
        params![path, expected_source, platform_id, generation],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn save_settings_if_revision_with_store(
    conn: &Connection,
    settings: &AppSettings,
    expected_revision: i64,
    credential_store: &dyn CredentialStore,
    staged_reference: &str,
) -> LocalResult<i64> {
    if expected_revision < 0 {
        return Err("settings revision 不能为负数。".into());
    }

    let normalized = normalize_settings(settings.clone());
    let current = conn
        .query_row(
            "SELECT COALESCE(api_token_ref, ''), settings_revision
             FROM app_settings WHERE id = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "设置记录不存在。".to_string())?;
    if current.1 != expected_revision {
        return Err(settings_conflict(expected_revision));
    }

    let credential = prepare_api_token(
        credential_store,
        &current.0,
        &normalized.api_token,
        staged_reference,
    )?;
    let next_revision = expected_revision.saturating_add(1);
    let updated = match conn.execute(
        "UPDATE app_settings SET
                theme_mode = ?1,
                liquid_glass_style = ?2,
                accent_color = ?3,
                locale = ?4,
                backend_url = ?5,
                api_token = '',
                api_token_ref = ?6,
                processing_mode = ?7,
                settings_revision = settings_revision + 1,
                default_hotwords = ?8,
                summary_template = ?9,
                concurrency = ?10,
                python_path = ?11,
                ffmpeg_path = ?12,
                python_runtime_source = ?13,
                ffmpeg_runtime_source = ?14,
                runner_script_path = ?15,
                local_asr_device = ?16,
                local_asr_threads = ?17,
                local_asr_batch_size_seconds = ?18,
                runtime_download_source = ?19
             WHERE id = 1 AND settings_revision = ?20",
        params![
            normalized.theme_mode,
            normalized.liquid_glass_style,
            normalized.accent_color,
            normalized.locale,
            normalized.backend_url,
            credential.target_reference,
            normalized.processing_mode,
            normalized.default_hotwords,
            normalized.summary_template,
            i64::from(normalized.concurrency),
            normalized.python_path,
            normalized.ffmpeg_path,
            normalized.python_runtime_source,
            normalized.ffmpeg_runtime_source,
            normalized.runner_script_path,
            normalized.local_asr_device,
            i64::from(normalized.local_asr_threads),
            i64::from(normalized.local_asr_batch_size_seconds),
            normalized.runtime_download_source,
            expected_revision,
        ],
    ) {
        Ok(updated) => updated,
        Err(error) => {
            return match settings_reference_matches(
                conn,
                next_revision,
                &credential.target_reference,
            ) {
                Ok(true) => {
                    finalize_api_token(credential_store, &credential);
                    Ok(next_revision)
                }
                Ok(false) => Err(rollback_api_token(
                    credential_store,
                    &credential,
                    error.to_string(),
                )),
                Err(check_error) => Err(format!(
                    "保存设置失败且无法确认提交结果: {error}; {check_error}"
                )),
            };
        }
    };
    if updated == 0 {
        return Err(rollback_api_token(
            credential_store,
            &credential,
            settings_conflict(expected_revision),
        ));
    }
    finalize_api_token(credential_store, &credential);
    Ok(next_revision)
}

struct PreparedApiToken {
    previous_reference: String,
    target_reference: String,
    staged_reference: Option<String>,
}

fn prepare_api_token(
    credential_store: &dyn CredentialStore,
    previous_reference: &str,
    api_token: &str,
    staged_reference: &str,
) -> LocalResult<PreparedApiToken> {
    if api_token.is_empty() {
        return Ok(PreparedApiToken {
            previous_reference: previous_reference.to_string(),
            target_reference: String::new(),
            staged_reference: None,
        });
    }
    if !previous_reference.is_empty()
        && credential_store
            .get_secret(previous_reference)
            .map_err(String::from)?
            .as_deref()
            == Some(api_token)
    {
        return Ok(PreparedApiToken {
            previous_reference: previous_reference.to_string(),
            target_reference: previous_reference.to_string(),
            staged_reference: None,
        });
    }
    if staged_reference.is_empty() || staged_reference == previous_reference {
        return Err("暂存凭据引用无效。".into());
    }
    credential_store
        .set_secret(staged_reference, api_token)
        .map_err(String::from)?;
    Ok(PreparedApiToken {
        previous_reference: previous_reference.to_string(),
        target_reference: staged_reference.to_string(),
        staged_reference: Some(staged_reference.to_string()),
    })
}

fn finalize_api_token(credential_store: &dyn CredentialStore, credential: &PreparedApiToken) {
    if !credential.previous_reference.is_empty()
        && credential.previous_reference != credential.target_reference
    {
        let _ = credential_store.delete_secret(&credential.previous_reference);
    }
}

fn rollback_api_token(
    credential_store: &dyn CredentialStore,
    credential: &PreparedApiToken,
    error: String,
) -> String {
    let Some(staged_reference) = credential.staged_reference.as_deref() else {
        return error;
    };
    match credential_store.delete_secret(staged_reference) {
        Ok(()) => error,
        Err(cleanup_error) => format!("{error}; 清理暂存凭据失败: {cleanup_error}"),
    }
}

fn settings_reference_matches(
    conn: &Connection,
    revision: i64,
    credential_reference: &str,
) -> LocalResult<bool> {
    conn.query_row(
        "SELECT 1 FROM app_settings
         WHERE id = 1 AND settings_revision = ?1 AND api_token_ref = ?2",
        params![revision, credential_reference],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|error| error.to_string())
}

fn settings_conflict(expected_revision: i64) -> String {
    format!("设置已被其他窗口更新；期望 revision {expected_revision}，请刷新后重试。")
}

fn next_api_token_reference(revision: i64) -> String {
    format!(
        "{}:v{}:{}",
        credential_key_for_remote_api_token(),
        revision,
        ids::timestamped_id("write")
    )
}

fn normalize_settings(mut settings: AppSettings) -> AppSettings {
    settings.theme_mode = match settings.theme_mode.as_str() {
        "light" => "light".into(),
        "dark" => "dark".into(),
        _ => "auto".into(),
    };
    settings.liquid_glass_style = match settings.liquid_glass_style.as_str() {
        "tinted" => "tinted".into(),
        _ => "transparent".into(),
    };
    settings.locale = match settings.locale.as_str() {
        "en-US" => "en-US".into(),
        _ => "zh-CN".into(),
    };
    if !is_valid_hex_color(&settings.accent_color) {
        settings.accent_color = "#2f6dff".into();
    } else {
        settings.accent_color = settings.accent_color.trim().to_lowercase();
    }
    settings.backend_url = settings.backend_url.trim().to_string();
    settings.api_token = settings.api_token.trim().to_string();
    settings.processing_mode = match settings.processing_mode.trim() {
        "remote" => "remote".into(),
        _ => "local".into(),
    };
    settings.default_hotwords = settings.default_hotwords.trim().to_string();
    settings.summary_template = settings.summary_template.trim().to_string();
    settings.concurrency = settings.concurrency.clamp(1, 8);
    settings.python_path = settings.python_path.trim().to_string();
    settings.ffmpeg_path = settings.ffmpeg_path.trim().to_string();
    settings.python_runtime_source = normalize_runtime_source(&settings.python_runtime_source);
    settings.ffmpeg_runtime_source = normalize_runtime_source(&settings.ffmpeg_runtime_source);
    settings.runner_script_path = settings.runner_script_path.trim().to_string();
    settings.local_asr_device = match settings.local_asr_device.as_str() {
        "cpu" => "cpu".into(),
        "mps" => "mps".into(),
        "cuda" => "cuda".into(),
        _ => "auto".into(),
    };
    settings.local_asr_threads = settings.local_asr_threads.min(32);
    settings.local_asr_batch_size_seconds = settings.local_asr_batch_size_seconds.clamp(30, 1200);
    settings.runtime_download_source = settings.runtime_download_source.trim().to_string();
    settings
}

fn normalize_runtime_source(value: &str) -> String {
    if value.trim() == "system" {
        "system".into()
    } else {
        "managed".into()
    }
}

fn is_valid_hex_color(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|char| char.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{
        load_settings, load_settings_with_revision, load_ui_preferences,
        publish_detected_runtime_path, save_settings_if_revision,
        save_settings_if_revision_with_store, save_settings_with_store,
        set_runtime_component_source,
    };
    use crate::{
        domain::error::AppError,
        infrastructure::credentials::{CredentialResult, CredentialStore},
        local_db::AppSettings,
    };
    use rusqlite::Connection;
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        fs,
        path::PathBuf,
    };

    #[derive(Default)]
    struct TestCredentialStore {
        secrets: RefCell<HashMap<String, String>>,
        race_database: RefCell<Option<PathBuf>>,
        race_once: Cell<bool>,
    }

    impl CredentialStore for TestCredentialStore {
        fn get_secret(&self, key: &str) -> CredentialResult<Option<String>> {
            Ok(self.secrets.borrow().get(key).cloned())
        }

        fn set_secret(&self, key: &str, value: &str) -> CredentialResult<()> {
            self.secrets
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
            if !self.race_once.replace(true) {
                if let Some(path) = self.race_database.borrow().as_ref() {
                    let conn = Connection::open(path)
                        .map_err(|error| AppError::Infrastructure(error.to_string()))?;
                    conn.execute(
                        "UPDATE app_settings
                         SET settings_revision = settings_revision + 1,
                             theme_mode = 'dark'
                         WHERE id = 1",
                        [],
                    )
                    .map_err(|error| AppError::Infrastructure(error.to_string()))?;
                }
            }
            Ok(())
        }

        fn delete_secret(&self, key: &str) -> CredentialResult<()> {
            self.secrets.borrow_mut().remove(key);
            Ok(())
        }
    }

    #[test]
    fn runtime_executable_paths_round_trip_independently() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(
            "CREATE TABLE app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                theme_mode TEXT NOT NULL,
                liquid_glass_style TEXT NOT NULL,
                accent_color TEXT NOT NULL,
                locale TEXT NOT NULL,
                backend_url TEXT NOT NULL,
                api_token TEXT NOT NULL,
                api_token_ref TEXT NOT NULL,
                processing_mode TEXT NOT NULL,
                settings_revision INTEGER NOT NULL,
                default_hotwords TEXT NOT NULL,
                summary_template TEXT NOT NULL,
                concurrency INTEGER NOT NULL,
                python_path TEXT NOT NULL,
                ffmpeg_path TEXT NOT NULL,
                python_runtime_source TEXT NOT NULL,
                ffmpeg_runtime_source TEXT NOT NULL,
                runner_script_path TEXT NOT NULL,
                local_asr_device TEXT NOT NULL,
                local_asr_threads INTEGER NOT NULL,
                local_asr_batch_size_seconds INTEGER NOT NULL,
                runtime_download_source TEXT NOT NULL
            );",
        )
        .expect("settings schema");

        let settings = AppSettings {
            python_path: "  /opt/tools/python3  ".into(),
            ffmpeg_path: "  /opt/tools/ffmpeg  ".into(),
            processing_mode: "remote".into(),
            ..AppSettings::default()
        };
        save_settings_with_store(
            &conn,
            &settings,
            &TestCredentialStore::default(),
            "settings:test:initial",
        )
        .expect("save system runtimes");

        let loaded = load_settings(&conn).expect("load system runtimes");
        assert_eq!(loaded.python_path, "/opt/tools/python3");
        assert_eq!(loaded.ffmpeg_path, "/opt/tools/ffmpeg");
        assert_eq!(loaded.python_runtime_source, "managed");
        assert_eq!(loaded.ffmpeg_runtime_source, "managed");
        assert_eq!(loaded.processing_mode, "remote");

        let (_, revision) = load_settings_with_revision(&conn).expect("load revision");
        save_settings_if_revision(
            &conn,
            &AppSettings {
                python_path: String::new(),
                ffmpeg_path: loaded.ffmpeg_path,
                ..loaded
            },
            revision,
        )
        .expect("switch only Python back to managed");

        let loaded = load_settings(&conn).expect("reload mixed runtime sources");
        assert!(loaded.python_path.is_empty());
        assert_eq!(loaded.ffmpeg_path, "/opt/tools/ffmpeg");
    }

    #[test]
    fn ui_preferences_query_returns_only_appearance_fields() {
        let conn = Connection::open_in_memory().expect("database");
        create_settings_table(&conn);
        conn.execute(
            "INSERT INTO app_settings (
               id, theme_mode, liquid_glass_style, accent_color, locale, backend_url,
               api_token, api_token_ref, processing_mode, settings_revision,
               default_hotwords, summary_template, concurrency, python_path, ffmpeg_path,
               python_runtime_source, ffmpeg_runtime_source, runner_script_path,
               local_asr_device, local_asr_threads, local_asr_batch_size_seconds,
               runtime_download_source
             ) VALUES (1, 'dark', 'tinted', '#123456', 'en-US',
               'https://secret.example', 'plaintext-secret', 'credential:secret', 'remote', 9,
               'secret words', 'secret template', 8, '/secret/python', '/secret/ffmpeg',
               'system', 'system', '/secret/runner', 'cuda', 16, 600, 'secret-source')",
            [],
        )
        .expect("settings row");

        let preferences = load_ui_preferences(&conn).expect("ui preferences");
        let serialized = serde_json::to_value(preferences).expect("serialize preferences");

        assert_eq!(
            serialized,
            serde_json::json!({
                "themeMode": "dark",
                "liquidGlassStyle": "tinted",
                "accentColor": "#123456",
                "locale": "en-US"
            })
        );
    }

    #[test]
    fn ui_preferences_use_safe_defaults_without_creating_settings() {
        let conn = Connection::open_in_memory().expect("database");
        create_settings_table(&conn);

        let preferences = load_ui_preferences(&conn).expect("default ui preferences");

        assert_eq!(preferences.theme_mode, "auto");
        assert_eq!(preferences.liquid_glass_style, "transparent");
        assert_eq!(preferences.accent_color, "#2f6dff");
        assert_eq!(preferences.locale, "zh-CN");
        let settings_count = conn
            .query_row("SELECT COUNT(*) FROM app_settings", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("settings count");
        assert_eq!(settings_count, 0);
    }

    #[test]
    fn stale_revision_cannot_overwrite_newer_settings() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(
            "CREATE TABLE app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                theme_mode TEXT NOT NULL, liquid_glass_style TEXT NOT NULL,
                accent_color TEXT NOT NULL, locale TEXT NOT NULL,
                backend_url TEXT NOT NULL, api_token TEXT NOT NULL,
                api_token_ref TEXT NOT NULL, processing_mode TEXT NOT NULL,
                settings_revision INTEGER NOT NULL, default_hotwords TEXT NOT NULL,
                summary_template TEXT NOT NULL, concurrency INTEGER NOT NULL,
                python_path TEXT NOT NULL, ffmpeg_path TEXT NOT NULL,
                python_runtime_source TEXT NOT NULL, ffmpeg_runtime_source TEXT NOT NULL,
                runner_script_path TEXT NOT NULL, local_asr_device TEXT NOT NULL,
                local_asr_threads INTEGER NOT NULL, local_asr_batch_size_seconds INTEGER NOT NULL,
                runtime_download_source TEXT NOT NULL
            );",
        )
        .expect("settings schema");
        save_settings_with_store(
            &conn,
            &AppSettings::default(),
            &TestCredentialStore::default(),
            "settings:test:initial",
        )
        .expect("initial settings");

        let first = AppSettings {
            theme_mode: "dark".into(),
            ..AppSettings::default()
        };
        assert_eq!(save_settings_if_revision(&conn, &first, 0), Ok(1));

        let stale = AppSettings {
            theme_mode: "light".into(),
            ..AppSettings::default()
        };
        let error = save_settings_if_revision(&conn, &stale, 0).expect_err("stale save");
        assert!(error.contains("其他窗口更新"));
        let (loaded, revision) = load_settings_with_revision(&conn).expect("current settings");
        assert_eq!(loaded.theme_mode, "dark");
        assert_eq!(revision, 1);
    }

    #[test]
    fn runtime_patches_preserve_concurrent_ui_settings() {
        let path = temp_database_path("runtime-settings-patch");
        let runtime_conn = Connection::open(&path).expect("runtime database");
        create_settings_table(&runtime_conn);
        runtime_conn
            .execute_batch(
                "CREATE TABLE runtime_component_state (
                   platform_id TEXT NOT NULL,
                   component TEXT NOT NULL,
                   source TEXT NOT NULL,
                   operation_generation INTEGER NOT NULL,
                   PRIMARY KEY(platform_id, component, source)
                 );
                 INSERT INTO app_settings (
                   id, theme_mode, liquid_glass_style, accent_color, locale, backend_url,
                   api_token, api_token_ref, processing_mode, settings_revision,
                   default_hotwords, summary_template, concurrency, python_path, ffmpeg_path,
                   python_runtime_source, ffmpeg_runtime_source, runner_script_path,
                   local_asr_device, local_asr_threads, local_asr_batch_size_seconds,
                   runtime_download_source
                 ) VALUES (1, 'auto', 'transparent', '#2f6dff', 'zh-CN',
                   'https://old.example', '', 'settings:test:old', 'local', 0, '', '', 2,
                   '/old/python', '', 'managed', 'managed', '', 'auto', 0, 300, '');
                 INSERT INTO runtime_component_state
                   (platform_id, component, source, operation_generation)
                 VALUES ('darwin-aarch64', 'python', 'system', 7);",
            )
            .expect("runtime settings fixture");
        let ui_conn = Connection::open(&path).expect("ui database");
        ui_conn
            .execute(
                "UPDATE app_settings
                 SET theme_mode = 'dark', backend_url = 'https://new.example',
                     api_token_ref = 'settings:test:new', settings_revision = settings_revision + 1
                 WHERE id = 1",
                [],
            )
            .expect("concurrent ui save");

        assert_eq!(
            set_runtime_component_source(&runtime_conn, "python", "system"),
            Ok(2)
        );
        assert_eq!(
            publish_detected_runtime_path(
                &runtime_conn,
                "darwin-aarch64",
                "python",
                "/usr/bin/python3",
                "system",
                7,
            ),
            Ok(Some(3))
        );

        let persisted = runtime_conn
            .query_row(
                "SELECT theme_mode, backend_url, api_token_ref, python_runtime_source,
                        python_path, settings_revision
                 FROM app_settings WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .expect("patched settings");
        assert_eq!(
            persisted,
            (
                "dark".into(),
                "https://new.example".into(),
                "settings:test:new".into(),
                "system".into(),
                "/usr/bin/python3".into(),
                3,
            )
        );
        drop(ui_conn);
        drop(runtime_conn);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn detected_path_rejects_changed_source_or_generation() {
        let conn = Connection::open_in_memory().expect("database");
        create_settings_table(&conn);
        conn.execute_batch(
            "CREATE TABLE runtime_component_state (
               platform_id TEXT NOT NULL,
               component TEXT NOT NULL,
               source TEXT NOT NULL,
               operation_generation INTEGER NOT NULL,
               PRIMARY KEY(platform_id, component, source)
             );
             INSERT INTO app_settings (
               id, theme_mode, liquid_glass_style, accent_color, locale, backend_url,
               api_token, api_token_ref, processing_mode, settings_revision,
               default_hotwords, summary_template, concurrency, python_path, ffmpeg_path,
               python_runtime_source, ffmpeg_runtime_source, runner_script_path,
               local_asr_device, local_asr_threads, local_asr_batch_size_seconds,
               runtime_download_source
             ) VALUES (1, 'auto', 'transparent', '#2f6dff', 'zh-CN', '', '', '',
               'local', 4, '', '', 2, '/kept/python', '', 'managed', 'managed', '',
               'auto', 0, 300, '');
             INSERT INTO runtime_component_state
               (platform_id, component, source, operation_generation)
             VALUES ('darwin-aarch64', 'python', 'system', 8);",
        )
        .expect("runtime settings fixture");

        assert_eq!(
            publish_detected_runtime_path(
                &conn,
                "darwin-aarch64",
                "python",
                "/late/python",
                "system",
                8,
            ),
            Ok(None)
        );
        conn.execute(
            "UPDATE app_settings SET python_runtime_source = 'system' WHERE id = 1",
            [],
        )
        .expect("select system runtime");
        assert_eq!(
            publish_detected_runtime_path(
                &conn,
                "darwin-aarch64",
                "python",
                "/stale/python",
                "system",
                7,
            ),
            Ok(None)
        );
        let persisted = conn
            .query_row(
                "SELECT python_path, settings_revision FROM app_settings WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .expect("unchanged settings");
        assert_eq!(persisted, ("/kept/python".into(), 4));
    }

    #[test]
    fn staged_credential_is_cleaned_when_revision_changes_during_write() {
        let path = temp_database_path("settings-race");
        let conn = Connection::open(&path).expect("database");
        create_settings_table(&conn);
        conn.execute(
            "INSERT INTO app_settings (
               id, theme_mode, liquid_glass_style, accent_color, locale, backend_url,
               api_token, api_token_ref, processing_mode, settings_revision,
               default_hotwords, summary_template, concurrency, python_path, ffmpeg_path,
               python_runtime_source, ffmpeg_runtime_source, runner_script_path,
               local_asr_device, local_asr_threads, local_asr_batch_size_seconds,
               runtime_download_source
             ) VALUES (1, 'auto', 'transparent', '#2f6dff', 'zh-CN', '', '',
               'settings:test:old', 'local', 0, '', '', 2, '', '', 'managed',
               'managed', '', 'auto', 0, 300, '')",
            [],
        )
        .expect("settings row");
        let store = TestCredentialStore::default();
        store
            .secrets
            .borrow_mut()
            .insert("settings:test:old".into(), "old-token".into());
        *store.race_database.borrow_mut() = Some(path.clone());

        let settings = AppSettings {
            api_token: "new-token".into(),
            ..AppSettings::default()
        };
        let error = save_settings_if_revision_with_store(
            &conn,
            &settings,
            0,
            &store,
            "settings:test:staged",
        )
        .expect_err("racing update must win");

        assert!(error.contains("其他窗口更新"));
        assert_eq!(
            store.secrets.borrow().get("settings:test:old"),
            Some(&"old-token".to_string())
        );
        assert!(!store.secrets.borrow().contains_key("settings:test:staged"));
        let persisted = conn
            .query_row(
                "SELECT api_token_ref, settings_revision, theme_mode
                 FROM app_settings WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .expect("persisted settings");
        assert_eq!(persisted, ("settings:test:old".into(), 1, "dark".into()));
        drop(conn);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn staged_credential_is_cleaned_when_database_update_fails() {
        let conn = Connection::open_in_memory().expect("database");
        create_settings_table(&conn);
        conn.execute_batch(
            "INSERT INTO app_settings (
               id, theme_mode, liquid_glass_style, accent_color, locale, backend_url,
               api_token, api_token_ref, processing_mode, settings_revision,
               default_hotwords, summary_template, concurrency, python_path, ffmpeg_path,
               python_runtime_source, ffmpeg_runtime_source, runner_script_path,
               local_asr_device, local_asr_threads, local_asr_batch_size_seconds,
               runtime_download_source
             ) VALUES (1, 'auto', 'transparent', '#2f6dff', 'zh-CN', '', '',
               'settings:test:old', 'local', 0, '', '', 2, '', '', 'managed',
               'managed', '', 'auto', 0, 300, '');
             CREATE TRIGGER reject_settings_update BEFORE UPDATE ON app_settings
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        )
        .expect("settings fixture");
        let store = TestCredentialStore::default();
        store
            .secrets
            .borrow_mut()
            .insert("settings:test:old".into(), "old-token".into());

        let error = save_settings_if_revision_with_store(
            &conn,
            &AppSettings {
                api_token: "new-token".into(),
                ..AppSettings::default()
            },
            0,
            &store,
            "settings:test:staged",
        )
        .expect_err("trigger must reject update");

        assert!(error.contains("injected write failure"));
        assert_eq!(
            store.secrets.borrow().get("settings:test:old"),
            Some(&"old-token".to_string())
        );
        assert!(!store.secrets.borrow().contains_key("settings:test:staged"));
        let reference = conn
            .query_row(
                "SELECT api_token_ref FROM app_settings WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("old reference");
        assert_eq!(reference, "settings:test:old");
    }

    fn create_settings_table(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE app_settings (
               id INTEGER PRIMARY KEY CHECK (id = 1),
               theme_mode TEXT NOT NULL, liquid_glass_style TEXT NOT NULL,
               accent_color TEXT NOT NULL, locale TEXT NOT NULL,
               backend_url TEXT NOT NULL, api_token TEXT NOT NULL,
               api_token_ref TEXT NOT NULL, processing_mode TEXT NOT NULL,
               settings_revision INTEGER NOT NULL, default_hotwords TEXT NOT NULL,
               summary_template TEXT NOT NULL, concurrency INTEGER NOT NULL,
               python_path TEXT NOT NULL, ffmpeg_path TEXT NOT NULL,
               python_runtime_source TEXT NOT NULL, ffmpeg_runtime_source TEXT NOT NULL,
               runner_script_path TEXT NOT NULL, local_asr_device TEXT NOT NULL,
               local_asr_threads INTEGER NOT NULL,
               local_asr_batch_size_seconds INTEGER NOT NULL,
               runtime_download_source TEXT NOT NULL
             );",
        )
        .expect("settings schema");
    }

    fn temp_database_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "liberty-{label}-{}-{}.sqlite3",
            std::process::id(),
            crate::infrastructure::ids::timestamped_id("test")
        ))
    }
}
