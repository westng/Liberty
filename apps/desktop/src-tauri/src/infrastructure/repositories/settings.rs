use rusqlite::{params, Connection, OptionalExtension};

use crate::local_db::{AppSettings, LocalResult};

pub fn load_settings(conn: &Connection) -> LocalResult<AppSettings> {
    let loaded = conn
        .query_row(
            "SELECT theme_mode, liquid_glass_style, accent_color, locale, backend_url,
                    api_token, default_hotwords, summary_template, concurrency,
                    python_path, runner_script_path, local_asr_device,
                    local_asr_threads, local_asr_batch_size_seconds, runtime_download_source
             FROM app_settings
             WHERE id = 1",
            [],
            |row| {
                Ok(AppSettings {
                    theme_mode: row.get(0)?,
                    liquid_glass_style: row.get(1)?,
                    accent_color: row.get(2)?,
                    locale: row.get(3)?,
                    backend_url: row.get(4)?,
                    api_token: row.get(5)?,
                    default_hotwords: row.get(6)?,
                    summary_template: row.get(7)?,
                    concurrency: row.get::<_, i64>(8)? as u32,
                    python_path: row.get(9)?,
                    runner_script_path: row.get(10)?,
                    local_asr_device: row.get(11)?,
                    local_asr_threads: row.get::<_, i64>(12)? as u32,
                    local_asr_batch_size_seconds: row.get::<_, i64>(13)? as u32,
                    runtime_download_source: row.get(14)?,
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())?;

    match loaded {
        Some(settings) => Ok(normalize_settings(settings)),
        None => {
            let settings = AppSettings::default();
            save_settings(conn, &settings)?;
            Ok(settings)
        }
    }
}

pub fn save_settings(conn: &Connection, settings: &AppSettings) -> LocalResult<()> {
    let normalized = normalize_settings(settings.clone());
    conn.execute(
        "INSERT INTO app_settings (
            id, theme_mode, liquid_glass_style, accent_color, locale, backend_url,
            api_token, default_hotwords, summary_template, concurrency, python_path,
            runner_script_path, local_asr_device, local_asr_threads, local_asr_batch_size_seconds,
            runtime_download_source
         ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT(id) DO UPDATE SET
            theme_mode = excluded.theme_mode,
            liquid_glass_style = excluded.liquid_glass_style,
            accent_color = excluded.accent_color,
            locale = excluded.locale,
            backend_url = excluded.backend_url,
            api_token = excluded.api_token,
            default_hotwords = excluded.default_hotwords,
            summary_template = excluded.summary_template,
            concurrency = excluded.concurrency,
            python_path = excluded.python_path,
            runner_script_path = excluded.runner_script_path,
            local_asr_device = excluded.local_asr_device,
            local_asr_threads = excluded.local_asr_threads,
            local_asr_batch_size_seconds = excluded.local_asr_batch_size_seconds,
            runtime_download_source = excluded.runtime_download_source",
        params![
            normalized.theme_mode,
            normalized.liquid_glass_style,
            normalized.accent_color,
            normalized.locale,
            normalized.backend_url,
            normalized.api_token,
            normalized.default_hotwords,
            normalized.summary_template,
            i64::from(normalized.concurrency),
            normalized.python_path,
            normalized.runner_script_path,
            normalized.local_asr_device,
            i64::from(normalized.local_asr_threads),
            i64::from(normalized.local_asr_batch_size_seconds),
            normalized.runtime_download_source
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
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
    settings.default_hotwords = settings.default_hotwords.trim().to_string();
    settings.summary_template = settings.summary_template.trim().to_string();
    settings.concurrency = settings.concurrency.clamp(1, 8);
    settings.python_path = settings.python_path.trim().to_string();
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

fn is_valid_hex_color(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|char| char.is_ascii_hexdigit())
}
