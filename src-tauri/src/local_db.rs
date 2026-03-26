use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager};

pub type LocalResult<T> = Result<T, String>;

const BUILTIN_TEMPLATE_TIMESTAMP: &str = "2026-03-26T00:00:00.000Z";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<String>,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSummary {
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub action_items: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub follow_ups: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSourceFile {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    pub size_label: String,
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryActionItem {
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub due_date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryResult {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub action_items: Vec<AiSummaryActionItem>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub follow_ups: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiModelConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub include_speaker_by_default: bool,
    pub include_timestamp_by_default: bool,
    pub builtin: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryRun {
    pub id: String,
    pub job_id: String,
    #[serde(default)]
    pub model_config_id: String,
    #[serde(default)]
    pub template_id: String,
    pub include_speaker: bool,
    pub include_timestamp: bool,
    #[serde(default)]
    pub extra_instructions: String,
    pub status: String,
    pub error_message: Option<String>,
    pub prompt_preview: Option<String>,
    pub raw_response: Option<String>,
    pub result: Option<AiSummaryResult>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeetingJob {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub source_files: Vec<MeetingSourceFile>,
    pub duration_minutes: u32,
    pub processing_started_at_ms: Option<u64>,
    pub processing_finished_at_ms: Option<u64>,
    pub processing_duration_seconds: Option<u32>,
    pub created_at: String,
    #[serde(default)]
    pub hotwords: Vec<String>,
    pub lang: String,
    pub enable_speaker: bool,
    pub summary_template: String,
    pub upload_status: String,
    pub asr_status: String,
    pub summary_status: String,
    pub overall_status: String,
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub transcript_segments: Vec<TranscriptSegment>,
    #[serde(default)]
    pub speaker_segments: Vec<TranscriptSegment>,
    #[serde(default)]
    pub summary: MeetingSummary,
    #[serde(default)]
    pub summary_runs: Vec<AiSummaryRun>,
    pub active_summary_run_id: Option<String>,
    #[serde(default)]
    pub export_formats: Vec<String>,
    pub last_exported_at: Option<String>,
    pub process_log: Option<String>,
    pub python_path: Option<String>,
    pub runner_script_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme_mode: String,
    pub liquid_glass_style: String,
    pub accent_color: String,
    pub locale: String,
    pub backend_url: String,
    pub api_token: String,
    pub default_hotwords: String,
    pub summary_template: String,
    pub concurrency: u32,
    pub python_path: String,
    pub runner_script_path: String,
    pub local_asr_device: String,
    pub local_asr_threads: u32,
    pub local_asr_batch_size_seconds: u32,
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
            default_hotwords: "SeACo-Paraformer, FunASR, 会议纪要".into(),
            summary_template: "默认会议纪要模板".into(),
            concurrency: 2,
            python_path: String::new(),
            runner_script_path: String::new(),
            local_asr_device: "auto".into(),
            local_asr_threads: 0,
            local_asr_batch_size_seconds: 300,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressSnapshot {
    stage: String,
    status_message: Option<String>,
    failure_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct LegacyRunnerResult {
    duration_minutes: Option<u32>,
    transcript_segments: Option<Vec<TranscriptSegment>>,
    speaker_segments: Option<Vec<TranscriptSegment>>,
    failure_reason: Option<String>,
}

pub fn init_database(app: &AppHandle) -> LocalResult<()> {
    let mut conn = open_connection(app)?;
    apply_schema(&conn)?;
    seed_builtin_templates(&conn)?;
    import_legacy_jobs(app, &mut conn)?;
    Ok(())
}

pub fn open_connection(app: &AppHandle) -> LocalResult<Connection> {
    let path = database_path(app)?;
    Connection::open(path).map_err(|err| err.to_string())
}

pub fn database_path(app: &AppHandle) -> LocalResult<PathBuf> {
    let data_root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| err.to_string())?;
    fs::create_dir_all(&data_root).map_err(|err| err.to_string())?;
    Ok(data_root.join("liberty.sqlite3"))
}

pub fn jobs_root(app: &AppHandle) -> LocalResult<PathBuf> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| err.to_string())?
        .join("jobs");
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    Ok(root)
}

pub fn job_dir(app: &AppHandle, job_id: &str) -> LocalResult<PathBuf> {
    Ok(jobs_root(app)?.join(job_id))
}

pub fn list_jobs(app: &AppHandle) -> LocalResult<Vec<MeetingJob>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id FROM jobs
             ORDER BY datetime(created_at) DESC, created_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;

    ids.into_iter()
        .map(|id| get_job(app, &id))
        .collect::<LocalResult<Vec<_>>>()
}

pub fn get_job(app: &AppHandle, job_id: &str) -> LocalResult<MeetingJob> {
    init_database(app)?;
    let conn = open_connection(app)?;
    load_job(&conn, job_id)
}

pub fn get_settings(app: &AppHandle) -> LocalResult<AppSettings> {
    init_database(app)?;
    let conn = open_connection(app)?;
    load_settings(&conn)
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    save_settings_inner(&conn, settings)
}

pub fn save_job_snapshot(app: &AppHandle, job: &MeetingJob) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    save_job_snapshot_tx(&tx, job)?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn update_job_statuses(
    app: &AppHandle,
    job_id: &str,
    asr_status: &str,
    summary_status: &str,
    overall_status: &str,
    failure_reason: Option<&str>,
) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE jobs
         SET asr_status = ?2,
             summary_status = ?3,
             overall_status = ?4,
             failure_reason = ?5
         WHERE id = ?1",
        params![job_id, asr_status, summary_status, overall_status, failure_reason],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn update_job_process_log(app: &AppHandle, job_id: &str, process_log: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE jobs SET process_log = ?2 WHERE id = ?1",
        params![job_id, process_log],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn replace_job_segments(
    app: &AppHandle,
    job_id: &str,
    transcript_segments: &[TranscriptSegment],
    speaker_segments: &[TranscriptSegment],
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    replace_segments_tx(&tx, job_id, "transcript", transcript_segments)?;
    replace_segments_tx(&tx, job_id, "speaker", speaker_segments)?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn rename_job_speaker(
    app: &AppHandle,
    job_id: &str,
    from_speaker: &str,
    to_speaker: &str,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let normalized_from = from_speaker.trim();
    let normalized_to = to_speaker.trim();

    if normalized_to.is_empty() {
        return Err("讲话人名称不能为空。".into());
    }

    if normalized_from.is_empty() {
        tx.execute(
            "UPDATE transcript_segments
             SET speaker = ?2
             WHERE job_id = ?1
               AND segment_type = 'speaker'
               AND (speaker IS NULL OR trim(speaker) = '')",
            params![job_id, normalized_to],
        )
        .map_err(|err| err.to_string())?;
    } else {
        tx.execute(
            "UPDATE transcript_segments
             SET speaker = ?3
             WHERE job_id = ?1
               AND segment_type = 'speaker'
               AND speaker = ?2",
            params![job_id, normalized_from, normalized_to],
        )
        .map_err(|err| err.to_string())?;
    }

    tx.commit().map_err(|err| err.to_string())
}

pub fn update_job_completion(
    app: &AppHandle,
    job_id: &str,
    duration_minutes: u32,
    processing_finished_at_ms: u64,
    processing_duration_seconds: Option<u32>,
    failure_reason: Option<&str>,
) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE jobs
         SET duration_minutes = ?2,
             processing_finished_at_ms = ?3,
             processing_duration_seconds = ?4,
             summary_status = ?5,
             asr_status = ?6,
             overall_status = ?7,
             failure_reason = ?8
         WHERE id = ?1",
        params![
            job_id,
            duration_minutes,
            processing_finished_at_ms as i64,
            processing_duration_seconds.map(i64::from),
            "idle",
            "completed",
            "completed",
            failure_reason
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn mark_job_processing_started(
    app: &AppHandle,
    job_id: &str,
    processing_started_at_ms: u64,
) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE jobs
         SET processing_started_at_ms = ?2,
             processing_finished_at_ms = NULL,
             processing_duration_seconds = NULL
         WHERE id = ?1",
        params![job_id, processing_started_at_ms as i64],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn update_job_failure(
    app: &AppHandle,
    job_id: &str,
    processing_finished_at_ms: u64,
    processing_duration_seconds: Option<u32>,
    failure_reason: &str,
) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE jobs
         SET processing_finished_at_ms = ?2,
             processing_duration_seconds = ?3,
             asr_status = 'failed',
             summary_status = 'idle',
             overall_status = 'failed',
             failure_reason = ?4
         WHERE id = ?1",
        params![
            job_id,
            processing_finished_at_ms as i64,
            processing_duration_seconds.map(i64::from),
            failure_reason
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn reset_job_for_retry(
    app: &AppHandle,
    job_id: &str,
    python_path: &str,
    runner_script_path: &str,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE jobs
         SET upload_status = ?2,
             asr_status = ?3,
             summary_status = ?4,
             overall_status = ?5,
             failure_reason = NULL,
             process_log = NULL,
             duration_minutes = 0,
             processing_started_at_ms = NULL,
             processing_finished_at_ms = NULL,
             processing_duration_seconds = NULL,
             python_path = ?6,
             runner_script_path = ?7
         WHERE id = ?1",
        params![
            job_id,
            "uploaded",
            "queued",
            "idle",
            "queued",
            python_path,
            runner_script_path
        ],
    )
    .map_err(|err| err.to_string())?;
    replace_segments_tx(&tx, job_id, "transcript", &[])?;
    replace_segments_tx(&tx, job_id, "speaker", &[])?;
    tx.execute(
        "DELETE FROM ai_summary_runs WHERE job_id = ?1",
        params![job_id],
    )
    .map_err(|err| err.to_string())?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn delete_job(app: &AppHandle, job_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    tx.execute("DELETE FROM ai_summary_runs WHERE job_id = ?1", params![job_id])
        .map_err(|err| err.to_string())?;
    tx.execute("DELETE FROM transcript_segments WHERE job_id = ?1", params![job_id])
        .map_err(|err| err.to_string())?;
    tx.execute("DELETE FROM job_source_files WHERE job_id = ?1", params![job_id])
        .map_err(|err| err.to_string())?;
    tx.execute("DELETE FROM jobs WHERE id = ?1", params![job_id])
        .map_err(|err| err.to_string())?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn list_ai_models(app: &AppHandle) -> LocalResult<Vec<AiModelConfig>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, base_url, api_key, model, enabled, is_default, created_at, updated_at
             FROM ai_model_configs
             ORDER BY datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
        Ok(AiModelConfig {
            id: row.get(0)?,
            name: row.get(1)?,
            base_url: row.get(2)?,
            api_key: row.get(3)?,
            model: row.get(4)?,
            enabled: row.get::<_, i64>(5)? != 0,
            is_default: row.get::<_, i64>(6)? != 0,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })
        .map_err(|err| err.to_string())?;
    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(items)
}

pub fn save_ai_model(app: &AppHandle, model: &AiModelConfig) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;

    if model.is_default {
        tx.execute("UPDATE ai_model_configs SET is_default = 0", [])
            .map_err(|err| err.to_string())?;
    }

    tx.execute(
        "INSERT INTO ai_model_configs (
            id, name, base_url, api_key, model, enabled, is_default, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            base_url = excluded.base_url,
            api_key = excluded.api_key,
            model = excluded.model,
            enabled = excluded.enabled,
            is_default = excluded.is_default,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            model.id,
            model.name,
            model.base_url,
            model.api_key,
            model.model,
            if model.enabled { 1 } else { 0 },
            if model.is_default { 1 } else { 0 },
            model.created_at,
            model.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;

    tx.commit().map_err(|err| err.to_string())
}

pub fn delete_ai_model(app: &AppHandle, model_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute("DELETE FROM ai_model_configs WHERE id = ?1", params![model_id])
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn list_ai_templates(app: &AppHandle) -> LocalResult<Vec<AiSummaryTemplate>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, prompt, include_speaker_by_default,
                    include_timestamp_by_default, builtin, created_at, updated_at
             FROM ai_summary_templates
             ORDER BY builtin DESC, datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
        Ok(AiSummaryTemplate {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            prompt: row.get(3)?,
            include_speaker_by_default: row.get::<_, i64>(4)? != 0,
            include_timestamp_by_default: row.get::<_, i64>(5)? != 0,
            builtin: row.get::<_, i64>(6)? != 0,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })
        .map_err(|err| err.to_string())?;
    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(items)
}

pub fn save_ai_template(app: &AppHandle, template: &AiSummaryTemplate) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "INSERT INTO ai_summary_templates (
            id, name, description, prompt, include_speaker_by_default,
            include_timestamp_by_default, builtin, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            prompt = excluded.prompt,
            include_speaker_by_default = excluded.include_speaker_by_default,
            include_timestamp_by_default = excluded.include_timestamp_by_default,
            builtin = excluded.builtin,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            template.id,
            template.name,
            template.description,
            template.prompt,
            if template.include_speaker_by_default { 1 } else { 0 },
            if template.include_timestamp_by_default { 1 } else { 0 },
            if template.builtin { 1 } else { 0 },
            template.created_at,
            template.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn delete_ai_template(app: &AppHandle, template_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let builtin = conn
        .query_row(
            "SELECT builtin FROM ai_summary_templates WHERE id = ?1",
            params![template_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?;

    if builtin.unwrap_or_default() != 0 {
        return Err("内置模板不可删除。".into());
    }

    conn.execute("DELETE FROM ai_summary_templates WHERE id = ?1", params![template_id])
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn list_ai_summary_runs(app: &AppHandle, job_id: &str) -> LocalResult<Vec<AiSummaryRun>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    load_summary_runs(&conn, job_id)
}

pub fn save_ai_summary_run(app: &AppHandle, run: &AiSummaryRun) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let result_json = run
        .result
        .as_ref()
        .map(|value| serde_json::to_string(value).map_err(|err| err.to_string()))
        .transpose()?;

    conn.execute(
        "INSERT INTO ai_summary_runs (
            id, job_id, model_config_id, template_id, include_speaker, include_timestamp,
            extra_instructions, status, error_message, prompt_preview, raw_response,
            result_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(id) DO UPDATE SET
            job_id = excluded.job_id,
            model_config_id = excluded.model_config_id,
            template_id = excluded.template_id,
            include_speaker = excluded.include_speaker,
            include_timestamp = excluded.include_timestamp,
            extra_instructions = excluded.extra_instructions,
            status = excluded.status,
            error_message = excluded.error_message,
            prompt_preview = excluded.prompt_preview,
            raw_response = excluded.raw_response,
            result_json = excluded.result_json,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            run.id,
            run.job_id,
            empty_to_null(&run.model_config_id),
            empty_to_null(&run.template_id),
            if run.include_speaker { 1 } else { 0 },
            if run.include_timestamp { 1 } else { 0 },
            run.extra_instructions,
            run.status,
            run.error_message,
            run.prompt_preview,
            run.raw_response,
            result_json,
            run.created_at,
            run.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;

    let summary_status = match run.status.as_str() {
        "running" => "summarizing",
        "completed" => "completed",
        "failed" => "failed",
        _ => "idle",
    };
    let next_active_summary_run_id = if run.status == "completed" && run.result.is_some() {
        Some(run.id.clone())
    } else {
        None
    };
    conn.execute(
        "UPDATE jobs
         SET summary_status = ?2,
             active_summary_run_id = COALESCE(?3, active_summary_run_id)
         WHERE id = ?1",
        params![run.job_id, summary_status, next_active_summary_run_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn set_active_ai_summary_run(app: &AppHandle, job_id: &str, run_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE jobs SET active_summary_run_id = ?2 WHERE id = ?1",
        params![job_id, run_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn delete_ai_summary_run(app: &AppHandle, job_id: &str, run_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "DELETE FROM ai_summary_runs WHERE job_id = ?1 AND id = ?2",
        params![job_id, run_id],
    )
    .map_err(|err| err.to_string())?;

    let remaining_runs = load_summary_runs(&conn, job_id)?;
    let next_active_run = remaining_runs
        .iter()
        .find(|run| run.status == "completed" && run.result.is_some())
        .cloned()
        .or_else(|| remaining_runs.first().cloned());
    let summary_status = if remaining_runs.iter().any(|run| run.status == "running") {
        "summarizing"
    } else if next_active_run
        .as_ref()
        .and_then(|run| run.result.as_ref())
        .is_some()
    {
        "completed"
    } else if remaining_runs.iter().any(|run| run.status == "failed") {
        "failed"
    } else {
        "idle"
    };

    conn.execute(
        "UPDATE jobs
         SET summary_status = ?2,
             active_summary_run_id = ?3
         WHERE id = ?1",
        params![
            job_id,
            summary_status,
            next_active_run.as_ref().map(|run| run.id.clone())
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn apply_schema(conn: &Connection) -> LocalResult<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS app_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS app_settings (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          theme_mode TEXT NOT NULL,
          liquid_glass_style TEXT NOT NULL,
          accent_color TEXT NOT NULL,
          locale TEXT NOT NULL,
          backend_url TEXT NOT NULL,
          api_token TEXT NOT NULL,
          default_hotwords TEXT NOT NULL,
          summary_template TEXT NOT NULL,
          concurrency INTEGER NOT NULL DEFAULT 2,
          python_path TEXT NOT NULL,
          runner_script_path TEXT NOT NULL,
          local_asr_device TEXT NOT NULL DEFAULT 'auto',
          local_asr_threads INTEGER NOT NULL DEFAULT 0,
          local_asr_batch_size_seconds INTEGER NOT NULL DEFAULT 300
        );

        CREATE TABLE IF NOT EXISTS jobs (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          created_at TEXT NOT NULL,
          duration_minutes INTEGER NOT NULL DEFAULT 0,
          lang TEXT NOT NULL,
          enable_speaker INTEGER NOT NULL DEFAULT 1,
          summary_template TEXT NOT NULL,
         upload_status TEXT NOT NULL,
         asr_status TEXT NOT NULL,
         summary_status TEXT NOT NULL,
         overall_status TEXT NOT NULL,
          processing_started_at_ms INTEGER,
          processing_finished_at_ms INTEGER,
          processing_duration_seconds INTEGER,
          failure_reason TEXT,
          process_log TEXT,
          python_path TEXT,
          runner_script_path TEXT,
          active_summary_run_id TEXT,
          last_exported_at TEXT,
          hotwords_json TEXT NOT NULL,
          export_formats_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS job_source_files (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          name TEXT NOT NULL,
          path TEXT,
          size_label TEXT NOT NULL,
          kind TEXT NOT NULL,
          FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS transcript_segments (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          segment_type TEXT NOT NULL,
          start_ms INTEGER NOT NULL,
          end_ms INTEGER NOT NULL,
          speaker TEXT,
          text TEXT NOT NULL,
          segment_order INTEGER NOT NULL,
          FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS ai_model_configs (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          base_url TEXT NOT NULL,
          api_key TEXT NOT NULL,
          model TEXT NOT NULL,
          enabled INTEGER NOT NULL DEFAULT 1,
          is_default INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ai_summary_templates (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          description TEXT NOT NULL,
          prompt TEXT NOT NULL,
          include_speaker_by_default INTEGER NOT NULL DEFAULT 1,
          include_timestamp_by_default INTEGER NOT NULL DEFAULT 1,
          builtin INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ai_summary_runs (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          model_config_id TEXT,
          template_id TEXT,
          include_speaker INTEGER NOT NULL DEFAULT 1,
          include_timestamp INTEGER NOT NULL DEFAULT 1,
          extra_instructions TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL,
          error_message TEXT,
          prompt_preview TEXT,
          raw_response TEXT,
          result_json TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_job_source_files_job_id ON job_source_files(job_id);
        CREATE INDEX IF NOT EXISTS idx_segments_job_id ON transcript_segments(job_id, segment_type, segment_order);
        CREATE INDEX IF NOT EXISTS idx_ai_runs_job_id ON ai_summary_runs(job_id, updated_at DESC);
        ",
    )
    .map_err(|err| err.to_string())?;

    match conn.execute("ALTER TABLE jobs ADD COLUMN active_summary_run_id TEXT", []) {
        Ok(_) => {}
        Err(err) if err.to_string().contains("duplicate column name") => {}
        Err(err) => return Err(err.to_string()),
    }

    for statement in [
        "ALTER TABLE jobs ADD COLUMN processing_started_at_ms INTEGER",
        "ALTER TABLE jobs ADD COLUMN processing_finished_at_ms INTEGER",
        "ALTER TABLE jobs ADD COLUMN processing_duration_seconds INTEGER",
    ] {
        match conn.execute(statement, []) {
            Ok(_) => {}
            Err(err) if err.to_string().contains("duplicate column name") => {}
            Err(err) => return Err(err.to_string()),
        }
    }

    for statement in [
        "ALTER TABLE app_settings ADD COLUMN local_asr_device TEXT NOT NULL DEFAULT 'auto'",
        "ALTER TABLE app_settings ADD COLUMN local_asr_threads INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE app_settings ADD COLUMN local_asr_batch_size_seconds INTEGER NOT NULL DEFAULT 300",
    ] {
        match conn.execute(statement, []) {
            Ok(_) => {}
            Err(err) if err.to_string().contains("duplicate column name") => {}
            Err(err) => return Err(err.to_string()),
        }
    }

    Ok(())
}

fn seed_builtin_templates(conn: &Connection) -> LocalResult<()> {
    let count = conn
        .query_row(
            "SELECT COUNT(*) FROM ai_summary_templates WHERE builtin = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| err.to_string())?;

    if count > 0 {
        return Ok(());
    }

    let templates = [
        AiSummaryTemplate {
            id: "builtin-standard-summary".into(),
            name: "标准会议纪要".into(),
            description: "输出摘要、议题、结论、行动项、风险与跟进事项。".into(),
            prompt: "你是资深会议纪要助手。请基于用户提供的会议转写内容输出结构化 JSON。\n\n要求：\n1. 只输出合法 JSON，不要输出 Markdown、解释或额外文本。\n2. 保持客观，不要编造原文中不存在的事实。\n3. 结果必须包含 title、overview、topics、decisions、actionItems、risks、followUps。\n4. actionItems 必须是数组，每项包含 task、owner、dueDate 三个字段；无法判断时 owner 和 dueDate 置为空字符串。\n5. topics、decisions、risks、followUps 都返回字符串数组。\n6. overview 用简洁中文概述会议重点。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: true,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
        AiSummaryTemplate {
            id: "builtin-decisions-actions".into(),
            name: "决策与待办".into(),
            description: "更强调最终决策、责任归属和后续执行。".into(),
            prompt: "你是会议行动项整理助手。请根据会议内容输出结构化 JSON。\n\n要求：\n1. 只输出合法 JSON。\n2. 重点提炼已确认的决策、待办事项、负责人和时间信息。\n3. 如果原文没有明确负责人或截止日期，请返回空字符串，不要猜测。\n4. 结果字段固定为 title、overview、topics、decisions、actionItems、risks、followUps。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: false,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
        AiSummaryTemplate {
            id: "builtin-project-weekly-review".into(),
            name: "项目周会总结".into(),
            description: "适合项目推进类会议，重点整理进展、风险和下一步。".into(),
            prompt: "你是项目周会总结助手。请把会议内容整理成结构化 JSON。\n\n要求：\n1. 只输出合法 JSON。\n2. overview 要覆盖进度、阻塞点和下一步方向。\n3. topics 聚焦当前进度与关键议题。\n4. risks 与 followUps 必须尽量完整。\n5. 结果字段固定为 title、overview、topics、decisions、actionItems、risks、followUps。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: false,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
        AiSummaryTemplate {
            id: "builtin-interview-notes".into(),
            name: "访谈整理".into(),
            description: "适合客户访谈、需求访谈或复盘访谈。".into(),
            prompt: "你是访谈内容整理助手。请把访谈内容整理成结构化 JSON。\n\n要求：\n1. 只输出合法 JSON。\n2. summary 需要突出受访者核心观点和关键诉求。\n3. topics 用于概括主题，decisions 记录明确共识，actionItems 记录后续动作。\n4. 结果字段固定为 title、overview、topics、decisions、actionItems、risks、followUps。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: true,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
    ];

    for template in templates {
        save_ai_template_inner(conn, &template)?;
    }

    Ok(())
}

fn save_ai_template_inner(conn: &Connection, template: &AiSummaryTemplate) -> LocalResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO ai_summary_templates (
            id, name, description, prompt, include_speaker_by_default,
            include_timestamp_by_default, builtin, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            template.id,
            template.name,
            template.description,
            template.prompt,
            if template.include_speaker_by_default { 1 } else { 0 },
            if template.include_timestamp_by_default { 1 } else { 0 },
            if template.builtin { 1 } else { 0 },
            template.created_at,
            template.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn load_settings(conn: &Connection) -> LocalResult<AppSettings> {
    let loaded = conn
        .query_row(
            "SELECT theme_mode, liquid_glass_style, accent_color, locale, backend_url,
                    api_token, default_hotwords, summary_template, concurrency,
                    python_path, runner_script_path, local_asr_device,
                    local_asr_threads, local_asr_batch_size_seconds
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
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())?;

    match loaded {
        Some(settings) => Ok(normalize_settings(settings)),
        None => {
            let settings = AppSettings::default();
            save_settings_inner(conn, &settings)?;
            Ok(settings)
        }
    }
}

fn save_settings_inner(conn: &Connection, settings: &AppSettings) -> LocalResult<()> {
    let normalized = normalize_settings(settings.clone());
    conn.execute(
        "INSERT INTO app_settings (
            id, theme_mode, liquid_glass_style, accent_color, locale, backend_url,
            api_token, default_hotwords, summary_template, concurrency, python_path,
            runner_script_path, local_asr_device, local_asr_threads, local_asr_batch_size_seconds
         ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
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
            local_asr_batch_size_seconds = excluded.local_asr_batch_size_seconds",
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
            i64::from(normalized.local_asr_batch_size_seconds)
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
    settings
}

fn is_valid_hex_color(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed
            .chars()
            .skip(1)
            .all(|char| char.is_ascii_hexdigit())
}

fn import_legacy_jobs(app: &AppHandle, conn: &mut Connection) -> LocalResult<()> {
    let imported = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'legacy_jobs_imported'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?;

    if imported.as_deref() == Some("1") {
        return Ok(());
    }

    let root = jobs_root(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;

    for entry in fs::read_dir(root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if let Ok(job) = read_legacy_job(&path) {
            save_job_snapshot_tx(&tx, &job)?;

            if has_summary_content(&job.summary) {
                let imported_run = imported_summary_run(&job);
                save_ai_summary_run_tx(&tx, &imported_run)?;
            }
        }
    }

    tx.execute(
        "INSERT INTO app_meta (key, value) VALUES ('legacy_jobs_imported', '1')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )
    .map_err(|err| err.to_string())?;
    tx.commit().map_err(|err| err.to_string())
}

fn read_legacy_job(job_dir: &Path) -> LocalResult<MeetingJob> {
    let job_path = job_dir.join("job.json");
    let raw = fs::read(&job_path).map_err(|err| err.to_string())?;
    let mut job: MeetingJob = serde_json::from_slice(&raw).map_err(|err| err.to_string())?;

    if let Ok(progress) = read_json::<ProgressSnapshot>(&job_dir.join("progress.json")) {
        apply_progress_snapshot(&mut job, &progress);
    }

    if let Ok(result) = read_json::<LegacyRunnerResult>(&job_dir.join("result.json")) {
        if !job.enable_speaker && job.transcript_segments.is_empty() {
            job.transcript_segments = result.transcript_segments.unwrap_or_default();
        }
        if job.speaker_segments.is_empty() {
            job.speaker_segments = result.speaker_segments.unwrap_or_default();
        }
        if job.duration_minutes == 0 {
            job.duration_minutes = result.duration_minutes.unwrap_or(0);
        }
        if job.failure_reason.is_none() {
            job.failure_reason = result.failure_reason;
        }
    }

    job.process_log = fs::read_to_string(job_dir.join("process.log"))
        .ok()
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty());
    job.summary_runs = Vec::new();
    job.active_summary_run_id = None;

    Ok(job)
}

fn apply_progress_snapshot(job: &mut MeetingJob, progress: &ProgressSnapshot) {
    match progress.stage.as_str() {
        "queued" => {
            job.upload_status = "uploaded".into();
            job.asr_status = "queued".into();
            job.summary_status = "idle".into();
            job.overall_status = "queued".into();
        }
        "transcribing" => {
            job.upload_status = "uploaded".into();
            job.asr_status = "transcribing".into();
            job.summary_status = "idle".into();
            job.overall_status = "transcribing".into();
        }
        "speaker_processing" => {
            job.upload_status = "uploaded".into();
            job.asr_status = "speaker_processing".into();
            job.summary_status = "idle".into();
            job.overall_status = "speaker_processing".into();
        }
        "completed" => {
            job.upload_status = "uploaded".into();
            job.asr_status = "completed".into();
            if job.summary_status == "queued" {
                job.summary_status = "idle".into();
            }
            job.overall_status = "completed".into();
        }
        "failed" => {
            job.asr_status = "failed".into();
            job.overall_status = "failed".into();
        }
        _ => {}
    }

    if let Some(reason) = progress
        .failure_reason
        .clone()
        .or_else(|| progress.status_message.clone())
    {
        if progress.stage == "failed" {
            job.failure_reason = Some(reason);
        }
    }
}

fn save_job_snapshot_tx(tx: &Transaction<'_>, job: &MeetingJob) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO jobs (
            id, title, created_at, duration_minutes, lang, enable_speaker,
            summary_template, upload_status, asr_status, summary_status, overall_status,
            processing_started_at_ms, processing_finished_at_ms, processing_duration_seconds,
            failure_reason, process_log, python_path, runner_script_path, active_summary_run_id,
            last_exported_at, hotwords_json, export_formats_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            created_at = excluded.created_at,
            duration_minutes = excluded.duration_minutes,
            lang = excluded.lang,
            enable_speaker = excluded.enable_speaker,
            summary_template = excluded.summary_template,
            upload_status = excluded.upload_status,
            asr_status = excluded.asr_status,
            summary_status = excluded.summary_status,
            overall_status = excluded.overall_status,
            processing_started_at_ms = excluded.processing_started_at_ms,
            processing_finished_at_ms = excluded.processing_finished_at_ms,
            processing_duration_seconds = excluded.processing_duration_seconds,
            failure_reason = excluded.failure_reason,
            process_log = excluded.process_log,
            python_path = excluded.python_path,
            runner_script_path = excluded.runner_script_path,
            active_summary_run_id = excluded.active_summary_run_id,
            last_exported_at = excluded.last_exported_at,
            hotwords_json = excluded.hotwords_json,
            export_formats_json = excluded.export_formats_json",
        params![
            job.id,
            job.title,
            job.created_at,
            i64::from(job.duration_minutes),
            job.lang,
            if job.enable_speaker { 1 } else { 0 },
            job.summary_template,
            job.upload_status,
            job.asr_status,
            job.summary_status,
            job.overall_status,
            job.processing_started_at_ms.map(|value| value as i64),
            job.processing_finished_at_ms.map(|value| value as i64),
            job.processing_duration_seconds.map(i64::from),
            job.failure_reason,
            job.process_log,
            job.python_path,
            job.runner_script_path,
            job.active_summary_run_id,
            job.last_exported_at,
            serde_json::to_string(&job.hotwords).map_err(|err| err.to_string())?,
            serde_json::to_string(&job.export_formats).map_err(|err| err.to_string())?
        ],
    )
    .map_err(|err| err.to_string())?;

    tx.execute("DELETE FROM job_source_files WHERE job_id = ?1", params![job.id])
        .map_err(|err| err.to_string())?;

    for file in &job.source_files {
        tx.execute(
            "INSERT INTO job_source_files (id, job_id, name, path, size_label, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![file.id, job.id, file.name, file.path, file.size_label, file.kind],
        )
        .map_err(|err| err.to_string())?;
    }

    replace_segments_tx(tx, &job.id, "transcript", &job.transcript_segments)?;
    replace_segments_tx(tx, &job.id, "speaker", &job.speaker_segments)?;

    Ok(())
}

fn replace_segments_tx(
    tx: &Transaction<'_>,
    job_id: &str,
    segment_type: &str,
    segments: &[TranscriptSegment],
) -> LocalResult<()> {
    tx.execute(
        "DELETE FROM transcript_segments WHERE job_id = ?1 AND segment_type = ?2",
        params![job_id, segment_type],
    )
    .map_err(|err| err.to_string())?;

    for (index, segment) in segments.iter().enumerate() {
        tx.execute(
            "INSERT INTO transcript_segments (
                id, job_id, segment_type, start_ms, end_ms, speaker, text, segment_order
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                segment_row_id(job_id, segment_type, &segment.id),
                job_id,
                segment_type,
                segment.start_ms as i64,
                segment.end_ms as i64,
                segment.speaker,
                segment.text,
                index as i64
            ],
        )
        .map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn load_job(conn: &Connection, job_id: &str) -> LocalResult<MeetingJob> {
    let base = conn
        .query_row(
            "SELECT id, title, created_at, duration_minutes, lang, enable_speaker,
                    summary_template, upload_status, asr_status, summary_status,
                    overall_status, processing_started_at_ms, processing_finished_at_ms,
                    processing_duration_seconds, failure_reason, process_log, python_path,
                    runner_script_path, active_summary_run_id, last_exported_at, hotwords_json, export_formats_json
             FROM jobs WHERE id = ?1",
            params![job_id],
            |row| {
                Ok(MeetingJob {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    duration_minutes: row.get::<_, i64>(3)? as u32,
                    created_at: row.get(2)?,
                    processing_started_at_ms: row.get::<_, Option<i64>>(11)?.map(|value| value as u64),
                    processing_finished_at_ms: row.get::<_, Option<i64>>(12)?.map(|value| value as u64),
                    processing_duration_seconds: row.get::<_, Option<i64>>(13)?.map(|value| value as u32),
                    hotwords: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(20)?)
                        .unwrap_or_default(),
                    lang: row.get(4)?,
                    enable_speaker: row.get::<_, i64>(5)? != 0,
                    summary_template: row.get(6)?,
                    upload_status: row.get(7)?,
                    asr_status: row.get(8)?,
                    summary_status: row.get(9)?,
                    overall_status: row.get(10)?,
                    failure_reason: row.get(14)?,
                    process_log: row.get(15)?,
                    python_path: row.get(16)?,
                    runner_script_path: row.get(17)?,
                    active_summary_run_id: row.get(18)?,
                    last_exported_at: row.get(19)?,
                    export_formats: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(21)?)
                        .unwrap_or_else(|_| vec!["txt".into(), "md".into(), "srt".into(), "docx".into()]),
                    ..MeetingJob::default()
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())?;

    let mut job = base.ok_or_else(|| "没有找到这个任务。".to_string())?;
    job.source_files = load_source_files(conn, &job.id)?;
    job.transcript_segments = load_segments(conn, &job.id, "transcript")?;
    job.speaker_segments = load_segments(conn, &job.id, "speaker")?;
    if job.duration_minutes == 0 {
        job.duration_minutes =
            derive_duration_minutes_from_segments(&job.transcript_segments, &job.speaker_segments)
                .unwrap_or(0);
    }
    job.summary_runs = load_summary_runs(conn, &job.id)?;
    let active_run = job
        .active_summary_run_id
        .as_ref()
        .and_then(|run_id| {
            job.summary_runs
                .iter()
                .find(|run| run.id == *run_id && run.status == "completed" && run.result.is_some())
                .cloned()
        })
        .or_else(|| {
            job.summary_runs
                .iter()
                .find(|run| run.status == "completed" && run.result.is_some())
                .cloned()
        })
        .or_else(|| {
            job.summary_runs
                .iter()
                .max_by(|left, right| left.updated_at.cmp(&right.updated_at))
                .cloned()
        });
    job.active_summary_run_id = active_run.as_ref().map(|run| run.id.clone());
    job.summary = active_run
        .and_then(|run| run.result)
        .map(summary_result_to_meeting_summary)
        .unwrap_or_default();

    if job.summary_runs.is_empty() && job.summary_status == "queued" {
        job.summary_status = "idle".into();
    }

    Ok(job)
}

fn load_source_files(conn: &Connection, job_id: &str) -> LocalResult<Vec<MeetingSourceFile>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, path, size_label, kind
             FROM job_source_files
             WHERE job_id = ?1
             ORDER BY rowid ASC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map(params![job_id], |row| {
        Ok(MeetingSourceFile {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            size_label: row.get(3)?,
            kind: row.get(4)?,
        })
    })
        .map_err(|err| err.to_string())?;
    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(items)
}

fn load_segments(conn: &Connection, job_id: &str, segment_type: &str) -> LocalResult<Vec<TranscriptSegment>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, start_ms, end_ms, speaker, text
             FROM transcript_segments
             WHERE job_id = ?1 AND segment_type = ?2
             ORDER BY segment_order ASC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map(params![job_id, segment_type], |row| {
        Ok(TranscriptSegment {
            id: row.get(0)?,
            start_ms: row.get::<_, i64>(1)? as u64,
            end_ms: row.get::<_, i64>(2)? as u64,
            speaker: row.get(3)?,
            text: row.get(4)?,
        })
    })
        .map_err(|err| err.to_string())?;
    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(items)
}

fn derive_duration_minutes_from_segments(
    transcript_segments: &[TranscriptSegment],
    speaker_segments: &[TranscriptSegment],
) -> Option<u32> {
    let max_end_ms = transcript_segments
        .iter()
        .chain(speaker_segments.iter())
        .map(|segment| segment.end_ms)
        .max()?;

    if max_end_ms == 0 {
        return None;
    }

    Some(((max_end_ms as f64) / 60_000.0).ceil() as u32)
}

fn load_summary_runs(conn: &Connection, job_id: &str) -> LocalResult<Vec<AiSummaryRun>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, job_id, COALESCE(model_config_id, ''), COALESCE(template_id, ''),
                    include_speaker, include_timestamp, extra_instructions, status,
                    error_message, prompt_preview, raw_response, result_json,
                    created_at, updated_at
             FROM ai_summary_runs
             WHERE job_id = ?1
             ORDER BY datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map(params![job_id], |row| {
        let result_json: Option<String> = row.get(11)?;
        let result = result_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<AiSummaryResult>(value).ok());

        Ok(AiSummaryRun {
            id: row.get(0)?,
            job_id: row.get(1)?,
            model_config_id: row.get(2)?,
            template_id: row.get(3)?,
            include_speaker: row.get::<_, i64>(4)? != 0,
            include_timestamp: row.get::<_, i64>(5)? != 0,
            extra_instructions: row.get(6)?,
            status: row.get(7)?,
            error_message: row.get(8)?,
            prompt_preview: row.get(9)?,
            raw_response: row.get(10)?,
            result,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
        })
    })
        .map_err(|err| err.to_string())?;
    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(items)
}

fn save_ai_summary_run_tx(tx: &Transaction<'_>, run: &AiSummaryRun) -> LocalResult<()> {
    let result_json = run
        .result
        .as_ref()
        .map(|value| serde_json::to_string(value).map_err(|err| err.to_string()))
        .transpose()?;

    tx.execute(
        "INSERT OR REPLACE INTO ai_summary_runs (
            id, job_id, model_config_id, template_id, include_speaker, include_timestamp,
            extra_instructions, status, error_message, prompt_preview, raw_response,
            result_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            run.id,
            run.job_id,
            empty_to_null(&run.model_config_id),
            empty_to_null(&run.template_id),
            if run.include_speaker { 1 } else { 0 },
            if run.include_timestamp { 1 } else { 0 },
            run.extra_instructions,
            run.status,
            run.error_message,
            run.prompt_preview,
            run.raw_response,
            result_json,
            run.created_at,
            run.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn summary_result_to_meeting_summary(result: AiSummaryResult) -> MeetingSummary {
    MeetingSummary {
        overview: result.overview,
        topics: result.topics,
        decisions: result.decisions,
        action_items: result
            .action_items
            .into_iter()
            .map(|item| {
                let suffix = [item.owner, item.due_date]
                    .into_iter()
                    .filter(|part| !part.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" / ");
                if suffix.is_empty() {
                    item.task
                } else {
                    format!("{}（{}）", item.task, suffix)
                }
            })
            .collect(),
        risks: result.risks,
        follow_ups: result.follow_ups,
    }
}

fn has_summary_content(summary: &MeetingSummary) -> bool {
    !summary.overview.trim().is_empty()
        || !summary.topics.is_empty()
        || !summary.decisions.is_empty()
        || !summary.action_items.is_empty()
        || !summary.risks.is_empty()
        || !summary.follow_ups.is_empty()
}

fn imported_summary_run(job: &MeetingJob) -> AiSummaryRun {
    AiSummaryRun {
        id: format!("imported-summary-{}", job.id),
        job_id: job.id.clone(),
        model_config_id: String::new(),
        template_id: String::new(),
        include_speaker: job.enable_speaker,
        include_timestamp: true,
        extra_instructions: String::new(),
        status: "completed".into(),
        error_message: None,
        prompt_preview: Some("Imported from legacy JSON task".into()),
        raw_response: None,
        result: Some(AiSummaryResult {
            title: job.title.clone(),
            overview: job.summary.overview.clone(),
            topics: job.summary.topics.clone(),
            decisions: job.summary.decisions.clone(),
            action_items: job
                .summary
                .action_items
                .iter()
                .map(|item| AiSummaryActionItem {
                    task: item.clone(),
                    owner: String::new(),
                    due_date: String::new(),
                })
                .collect(),
            risks: job.summary.risks.clone(),
            follow_ups: job.summary.follow_ups.clone(),
        }),
        created_at: job.created_at.clone(),
        updated_at: job.created_at.clone(),
    }
}

fn segment_row_id(job_id: &str, segment_type: &str, segment_id: &str) -> String {
    format!("{job_id}:{segment_type}:{segment_id}")
}

fn empty_to_null(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> LocalResult<T> {
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    serde_json::from_slice(&bytes).map_err(|err| err.to_string())
}
