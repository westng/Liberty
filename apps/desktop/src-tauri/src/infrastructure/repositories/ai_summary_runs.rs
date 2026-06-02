use rusqlite::{params, Connection, Transaction};

use crate::local_db::{AiSummaryResult, AiSummaryRun, LocalResult, MeetingMinutesPayload};

pub fn list_summary_runs(conn: &Connection, job_id: &str) -> LocalResult<Vec<AiSummaryRun>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, job_id, COALESCE(model_config_id, ''), COALESCE(template_id, ''),
                    include_speaker, include_timestamp, extra_instructions, status,
                    error_message, prompt_preview, raw_response, result_json,
                    minutes_payload_json,
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
            let minutes_payload_json: Option<String> = row.get(12)?;
            let minutes_payload = minutes_payload_json
                .as_deref()
                .and_then(|value| serde_json::from_str::<MeetingMinutesPayload>(value).ok());

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
                minutes_payload,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

pub fn save_summary_run(conn: &Connection, run: &AiSummaryRun) -> LocalResult<()> {
    let result_json = run
        .result
        .as_ref()
        .map(|value| serde_json::to_string(value).map_err(|err| err.to_string()))
        .transpose()?;
    let minutes_payload_json = run
        .minutes_payload
        .as_ref()
        .map(|value| serde_json::to_string(value).map_err(|err| err.to_string()))
        .transpose()?;

    conn.execute(
        "INSERT INTO ai_summary_runs (
            id, job_id, model_config_id, template_id, include_speaker, include_timestamp,
            extra_instructions, status, error_message, prompt_preview, raw_response,
            result_json, minutes_payload_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
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
            minutes_payload_json = excluded.minutes_payload_json,
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
            minutes_payload_json,
            run.created_at,
            run.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;

    Ok(())
}

pub fn save_summary_run_tx(tx: &Transaction<'_>, run: &AiSummaryRun) -> LocalResult<()> {
    let result_json = run
        .result
        .as_ref()
        .map(|value| serde_json::to_string(value).map_err(|err| err.to_string()))
        .transpose()?;
    let minutes_payload_json = run
        .minutes_payload
        .as_ref()
        .map(|value| serde_json::to_string(value).map_err(|err| err.to_string()))
        .transpose()?;

    tx.execute(
        "INSERT OR REPLACE INTO ai_summary_runs (
            id, job_id, model_config_id, template_id, include_speaker, include_timestamp,
            extra_instructions, status, error_message, prompt_preview, raw_response,
            result_json, minutes_payload_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            minutes_payload_json,
            run.created_at,
            run.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn delete_summary_run(conn: &Connection, job_id: &str, run_id: &str) -> LocalResult<()> {
    conn.execute(
        "DELETE FROM ai_summary_runs WHERE job_id = ?1 AND id = ?2",
        params![job_id, run_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn delete_summary_runs_for_job_tx(tx: &Transaction<'_>, job_id: &str) -> LocalResult<()> {
    tx.execute(
        "DELETE FROM ai_summary_runs WHERE job_id = ?1",
        params![job_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn update_job_summary_selection(
    conn: &Connection,
    job_id: &str,
    summary_status: &str,
    active_run_id: Option<String>,
) -> LocalResult<()> {
    conn.execute(
        "UPDATE jobs
         SET summary_status = ?2,
             active_summary_run_id = ?3
         WHERE id = ?1",
        params![job_id, summary_status, active_run_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn update_job_summary_status_after_save(
    conn: &Connection,
    run: &AiSummaryRun,
) -> LocalResult<()> {
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

fn empty_to_null(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
