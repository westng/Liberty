use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::{
    infrastructure::{
        migrations,
        repositories::ai_summary_run_models::{
            AiSummaryCompletion, AiSummaryExecutionRecord, AiSummaryPendingChunk,
            AiSummaryRunLease, NewAiSummaryExecution,
        },
    },
    local_db::{AiSummaryResult, AiSummaryRun, LocalResult, MeetingMinutesPayload},
};

pub fn migrate_v7_ai_summary_runs(transaction: &Transaction<'_>) -> LocalResult<()> {
    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS ai_summary_runs (
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
               minutes_payload_json TEXT,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL,
               FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
             );",
        )
        .map_err(|error| error.to_string())?;

    for (column, statement) in [
        (
            "transcript_revision",
            "ALTER TABLE ai_summary_runs ADD COLUMN transcript_revision TEXT NOT NULL DEFAULT ''",
        ),
        (
            "transcript_sha256",
            "ALTER TABLE ai_summary_runs ADD COLUMN transcript_sha256 TEXT NOT NULL DEFAULT ''",
        ),
        (
            "transcript_snapshot_json",
            "ALTER TABLE ai_summary_runs ADD COLUMN transcript_snapshot_json TEXT NOT NULL DEFAULT ''",
        ),
        (
            "execution_snapshot_json",
            "ALTER TABLE ai_summary_runs ADD COLUMN execution_snapshot_json TEXT NOT NULL DEFAULT ''",
        ),
        (
            "chunk_count",
            "ALTER TABLE ai_summary_runs ADD COLUMN chunk_count INTEGER NOT NULL DEFAULT 0 CHECK (chunk_count >= 0)",
        ),
        (
            "attempt_id",
            "ALTER TABLE ai_summary_runs ADD COLUMN attempt_id INTEGER NOT NULL DEFAULT 0 CHECK (attempt_id >= 0)",
        ),
        (
            "lease_token",
            "ALTER TABLE ai_summary_runs ADD COLUMN lease_token INTEGER NOT NULL DEFAULT 0 CHECK (lease_token >= 0)",
        ),
        (
            "diagnostics_json",
            "ALTER TABLE ai_summary_runs ADD COLUMN diagnostics_json TEXT",
        ),
        (
            "completed_at",
            "ALTER TABLE ai_summary_runs ADD COLUMN completed_at TEXT",
        ),
    ] {
        migrations::add_column_if_missing(
            transaction,
            "ai_summary_runs",
            column,
            statement,
        )?;
    }

    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS ai_summary_run_chunks (
               run_id TEXT NOT NULL,
               chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
               chunk_sha256 TEXT NOT NULL,
               user_prompt TEXT NOT NULL,
               status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed')),
               raw_response TEXT,
               structured_result_json TEXT,
               attempt_id INTEGER NOT NULL DEFAULT 0 CHECK (attempt_id >= 0),
               lease_token INTEGER NOT NULL DEFAULT 0 CHECK (lease_token >= 0),
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL,
               completed_at TEXT,
               PRIMARY KEY(run_id, chunk_index),
               FOREIGN KEY(run_id) REFERENCES ai_summary_runs(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_ai_summary_runs_completed
               ON ai_summary_runs(job_id, status, completed_at DESC, updated_at DESC, id DESC);
             CREATE INDEX IF NOT EXISTS idx_ai_summary_run_chunks_pending
               ON ai_summary_run_chunks(run_id, status, chunk_index);",
        )
        .map_err(|error| error.to_string())?;

    transaction
        .execute(
            "UPDATE ai_summary_runs
             SET completed_at = COALESCE(completed_at, updated_at)
             WHERE status = 'completed' AND completed_at IS NULL",
            [],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE ai_summary_runs
             SET status = 'failed',
                 error_message = COALESCE(
                   error_message,
                   '旧版本未完成的 AI 总结无法恢复，请重新生成。'
                 )
             WHERE status = 'running' AND execution_snapshot_json = ''",
            [],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE ai_summary_runs AS stale
             SET status = 'failed',
                 error_message = '检测到同一任务存在重复运行，已由较新的运行接管。'
             WHERE stale.status = 'running'
               AND EXISTS (
                 SELECT 1 FROM ai_summary_runs AS newer
                 WHERE newer.job_id = stale.job_id
                   AND newer.status = 'running'
                   AND (
                     newer.updated_at > stale.updated_at
                     OR (newer.updated_at = stale.updated_at AND newer.id > stale.id)
                   )
               )",
            [],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_summary_runs_one_running_per_job
               ON ai_summary_runs(job_id) WHERE status = 'running';",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE jobs
             SET summary_status = CASE
               WHEN EXISTS (
                 SELECT 1 FROM ai_summary_runs
                 WHERE job_id = jobs.id AND status = 'running'
               ) THEN 'summarizing'
               WHEN EXISTS (
                 SELECT 1 FROM ai_summary_runs
                 WHERE job_id = jobs.id AND status = 'completed' AND result_json IS NOT NULL
               ) THEN 'completed'
               WHEN EXISTS (
                 SELECT 1 FROM ai_summary_runs
                 WHERE job_id = jobs.id AND status = 'failed'
               ) THEN 'failed'
               ELSE 'idle'
             END
             WHERE EXISTS (
               SELECT 1 FROM ai_summary_runs WHERE job_id = jobs.id
             )",
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn list_summary_runs(conn: &Connection, job_id: &str) -> LocalResult<Vec<AiSummaryRun>> {
    let mut statement = conn
        .prepare(
            "SELECT id, job_id, COALESCE(model_config_id, ''), COALESCE(template_id, ''),
                    include_speaker, include_timestamp, extra_instructions, status,
                    error_message, prompt_preview, raw_response, result_json,
                    minutes_payload_json, created_at, updated_at
             FROM ai_summary_runs
             WHERE job_id = ?1
             ORDER BY datetime(updated_at) DESC, updated_at DESC, id DESC",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map(params![job_id], map_summary_run)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn get_summary_run(conn: &Connection, job_id: &str, run_id: &str) -> LocalResult<AiSummaryRun> {
    conn.query_row(
        "SELECT id, job_id, COALESCE(model_config_id, ''), COALESCE(template_id, ''),
                include_speaker, include_timestamp, extra_instructions, status,
                error_message, prompt_preview, raw_response, result_json,
                minutes_payload_json, created_at, updated_at
         FROM ai_summary_runs
         WHERE job_id = ?1 AND id = ?2",
        params![job_id, run_id],
        map_summary_run,
    )
    .optional()
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "没有找到指定的 AI 总结运行。".to_string())
}

pub fn get_running_summary_run(
    conn: &Connection,
    job_id: &str,
) -> LocalResult<Option<AiSummaryRun>> {
    conn.query_row(
        "SELECT id, job_id, COALESCE(model_config_id, ''), COALESCE(template_id, ''),
                include_speaker, include_timestamp, extra_instructions, status,
                error_message, prompt_preview, raw_response, result_json,
                minutes_payload_json, created_at, updated_at
         FROM ai_summary_runs
         WHERE job_id = ?1 AND status = 'running'
         ORDER BY updated_at DESC, id DESC
         LIMIT 1",
        params![job_id],
        map_summary_run,
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn list_recoverable_executions(conn: &Connection) -> LocalResult<Vec<(String, String)>> {
    let mut statement = conn
        .prepare(
            "SELECT job_id, id
             FROM ai_summary_runs
             WHERE status = 'running' AND execution_snapshot_json <> '' AND chunk_count > 0
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(|error| error.to_string())?;
    let recoverable = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(recoverable)
}

pub fn save_summary_run_tx(transaction: &Transaction<'_>, run: &AiSummaryRun) -> LocalResult<()> {
    save_summary_run_inner(transaction, run)
}

pub fn backfill_minutes_payload(
    conn: &mut Connection,
    job_id: &str,
    run_id: &str,
    payload: &MeetingMinutesPayload,
) -> LocalResult<bool> {
    let payload_json = serde_json::to_string(payload)
        .map_err(|error| format!("会议纪要 payload 序列化失败: {error}"))?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let current = transaction
        .query_row(
            "SELECT minutes_payload_json
             FROM ai_summary_runs
             WHERE id = ?1 AND job_id = ?2 AND status = 'completed'",
            params![run_id, job_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "只能回填已完成的 AI 总结运行。".to_string())?;

    if current
        .as_deref()
        .and_then(|value| serde_json::from_str::<MeetingMinutesPayload>(value).ok())
        .is_some()
    {
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(false);
    }

    transaction
        .execute(
            "UPDATE ai_summary_runs
             SET minutes_payload_json = ?3
             WHERE id = ?1 AND job_id = ?2 AND status = 'completed'",
            params![run_id, job_id, payload_json],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(true)
}

fn save_summary_run_inner(conn: &Connection, run: &AiSummaryRun) -> LocalResult<()> {
    let result_json = serialize_optional(run.result.as_ref())?;
    let minutes_payload_json = serialize_optional(run.minutes_payload.as_ref())?;
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
            bool_to_sql(run.include_speaker),
            bool_to_sql(run.include_timestamp),
            run.extra_instructions,
            run.status,
            run.error_message,
            run.prompt_preview,
            run.raw_response,
            result_json,
            minutes_payload_json,
            run.created_at,
            run.updated_at,
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn create_execution(
    conn: &mut Connection,
    run: &AiSummaryRun,
    execution: &NewAiSummaryExecution<'_>,
) -> LocalResult<AiSummaryRun> {
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let existing = transaction
        .query_row(
            "SELECT id, job_id, COALESCE(model_config_id, ''), COALESCE(template_id, ''),
                    include_speaker, include_timestamp, extra_instructions, status,
                    error_message, prompt_preview, raw_response, result_json,
                    minutes_payload_json, created_at, updated_at
             FROM ai_summary_runs
             WHERE job_id = ?1 AND status = 'running'
             ORDER BY updated_at DESC, id DESC
             LIMIT 1",
            params![run.job_id],
            map_summary_run,
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(existing) = existing {
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(existing);
    }
    let result_json = serialize_optional(run.result.as_ref())?;
    let minutes_payload_json = serialize_optional(run.minutes_payload.as_ref())?;
    transaction
        .execute(
            "INSERT INTO ai_summary_runs (
               id, job_id, model_config_id, template_id, include_speaker, include_timestamp,
               extra_instructions, status, error_message, prompt_preview, raw_response,
               result_json, minutes_payload_json, created_at, updated_at,
               transcript_revision, transcript_sha256, transcript_snapshot_json,
               execution_snapshot_json, chunk_count, attempt_id, lease_token
             ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, 'running', NULL, ?8, NULL,
               ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 0, 0
             )",
            params![
                run.id,
                run.job_id,
                empty_to_null(&run.model_config_id),
                empty_to_null(&run.template_id),
                bool_to_sql(run.include_speaker),
                bool_to_sql(run.include_timestamp),
                run.extra_instructions,
                run.prompt_preview,
                result_json,
                minutes_payload_json,
                run.created_at,
                run.updated_at,
                execution.transcript_revision,
                execution.transcript_sha256,
                execution.transcript_snapshot_json,
                execution.execution_snapshot_json,
                i64::try_from(execution.chunks.len())
                    .map_err(|_| "AI 总结分块数量超出数据库范围。".to_string())?,
            ],
        )
        .map_err(|error| error.to_string())?;

    for chunk in execution.chunks {
        transaction
            .execute(
                "INSERT INTO ai_summary_run_chunks (
                   run_id, chunk_index, chunk_sha256, user_prompt, status,
                   created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?5)",
                params![
                    run.id,
                    i64::try_from(chunk.index)
                        .map_err(|_| "AI 总结分块序号超出数据库范围。".to_string())?,
                    chunk.sha256,
                    chunk.user_prompt,
                    run.created_at,
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction
        .execute(
            "UPDATE jobs SET summary_status = 'summarizing' WHERE id = ?1",
            params![run.job_id],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(run.clone())
}

pub fn claim_execution(
    conn: &mut Connection,
    job_id: &str,
    run_id: &str,
    now: &str,
) -> LocalResult<Option<AiSummaryRunLease>> {
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let status = transaction
        .query_row(
            "SELECT status FROM ai_summary_runs WHERE job_id = ?1 AND id = ?2",
            params![job_id, run_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "没有找到指定的 AI 总结运行。".to_string())?;
    if status == "completed" {
        return Ok(None);
    }
    let competing_run = transaction
        .query_row(
            "SELECT id FROM ai_summary_runs
             WHERE job_id = ?1 AND status = 'running' AND id <> ?2
             LIMIT 1",
            params![job_id, run_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(competing_run) = competing_run {
        return Err(format!(
            "任务已有正在执行的 AI 总结运行 {competing_run}，请恢复该运行。"
        ));
    }

    let changed = transaction
        .execute(
            "UPDATE ai_summary_runs
             SET attempt_id = attempt_id + 1,
                 lease_token = lease_token + 1,
                 status = 'running',
                 error_message = NULL,
                 updated_at = ?3
             WHERE job_id = ?1 AND id = ?2
               AND status <> 'completed'
               AND execution_snapshot_json <> ''
               AND chunk_count > 0",
            params![job_id, run_id, now],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("AI 总结运行缺少可恢复的执行快照。".into());
    }
    let (attempt_id, lease_token) = transaction
        .query_row(
            "SELECT attempt_id, lease_token FROM ai_summary_runs WHERE id = ?1",
            params![run_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE jobs SET summary_status = 'summarizing' WHERE id = ?1",
            params![job_id],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(Some(AiSummaryRunLease {
        run_id: run_id.to_string(),
        attempt_id: to_u64(attempt_id, "attempt_id")?,
        lease_token: to_u64(lease_token, "lease_token")?,
    }))
}

pub fn load_execution(
    conn: &Connection,
    lease: &AiSummaryRunLease,
) -> LocalResult<AiSummaryExecutionRecord> {
    conn.query_row(
        "SELECT id, job_id, COALESCE(model_config_id, ''), transcript_revision,
                transcript_sha256, execution_snapshot_json, chunk_count
         FROM ai_summary_runs
         WHERE id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'",
        params![
            lease.run_id,
            as_i64(lease.attempt_id, "attempt_id")?,
            as_i64(lease.lease_token, "lease_token")?,
        ],
        |row| {
            let chunk_count = row.get::<_, i64>(6)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                chunk_count,
            ))
        },
    )
    .optional()
    .map_err(|error| error.to_string())?
    .map(
        |(
            run_id,
            job_id,
            model_config_id,
            transcript_revision,
            transcript_sha256,
            execution_snapshot_json,
            chunk_count,
        )|
         -> LocalResult<AiSummaryExecutionRecord> {
            Ok(AiSummaryExecutionRecord {
                run_id,
                job_id,
                model_config_id,
                transcript_revision,
                transcript_sha256,
                execution_snapshot_json,
                chunk_count: to_usize(chunk_count, "chunk_count")?,
            })
        },
    )
    .transpose()?
    .ok_or_else(|| "AI 总结运行租约已失效。".to_string())
}

pub fn list_pending_chunks(
    conn: &Connection,
    lease: &AiSummaryRunLease,
) -> LocalResult<Vec<AiSummaryPendingChunk>> {
    let mut statement = conn
        .prepare(
            "SELECT chunk_index, chunk_sha256, user_prompt
             FROM ai_summary_run_chunks
             WHERE run_id = ?1 AND status = 'pending'
               AND EXISTS (
                 SELECT 1 FROM ai_summary_runs
                 WHERE id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'
               )
             ORDER BY chunk_index ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(
            params![
                lease.run_id,
                as_i64(lease.attempt_id, "attempt_id")?,
                as_i64(lease.lease_token, "lease_token")?,
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?;
    rows.map(|row| {
        let (index, sha256, user_prompt) = row.map_err(|error| error.to_string())?;
        Ok(AiSummaryPendingChunk {
            index: to_usize(index, "chunk_index")?,
            sha256,
            user_prompt,
        })
    })
    .collect()
}

pub fn save_chunk_result(
    conn: &Connection,
    lease: &AiSummaryRunLease,
    chunk: &AiSummaryPendingChunk,
    raw_response: &str,
    structured_result_json: &str,
    now: &str,
) -> LocalResult<bool> {
    let changed = conn
        .execute(
            "UPDATE ai_summary_run_chunks
             SET status = 'completed', raw_response = ?5, structured_result_json = ?6,
                 attempt_id = ?2, lease_token = ?3, updated_at = ?7, completed_at = ?7
             WHERE run_id = ?1 AND chunk_index = ?4 AND chunk_sha256 = ?8
               AND status = 'pending'
               AND EXISTS (
                 SELECT 1 FROM ai_summary_runs
                 WHERE id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'
               )",
            params![
                lease.run_id,
                as_i64(lease.attempt_id, "attempt_id")?,
                as_i64(lease.lease_token, "lease_token")?,
                i64::try_from(chunk.index)
                    .map_err(|_| "AI 总结分块序号超出数据库范围。".to_string())?,
                raw_response,
                structured_result_json,
                now,
                chunk.sha256,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(changed == 1)
}

pub fn load_completed_chunk_responses(
    conn: &Connection,
    lease: &AiSummaryRunLease,
) -> LocalResult<Vec<String>> {
    let execution = load_execution(conn, lease)?;
    let mut statement = conn
        .prepare(
            "SELECT raw_response
             FROM ai_summary_run_chunks
             WHERE run_id = ?1 AND status = 'completed'
             ORDER BY chunk_index ASC",
        )
        .map_err(|error| error.to_string())?;
    let responses = statement
        .query_map(params![lease.run_id], |row| row.get::<_, Option<String>>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if responses.len() != execution.chunk_count {
        return Err(format!(
            "AI 总结分块尚未全部完成: {}/{}。",
            responses.len(),
            execution.chunk_count
        ));
    }
    Ok(responses)
}

pub fn complete_execution(
    conn: &mut Connection,
    lease: &AiSummaryRunLease,
    completion: &AiSummaryCompletion<'_>,
) -> LocalResult<bool> {
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let job_id = transaction
        .query_row(
            "SELECT job_id FROM ai_summary_runs
             WHERE id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'",
            params![
                lease.run_id,
                as_i64(lease.attempt_id, "attempt_id")?,
                as_i64(lease.lease_token, "lease_token")?,
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some(job_id) = job_id else {
        return Ok(false);
    };
    let changed = transaction
        .execute(
            "UPDATE ai_summary_runs
             SET status = 'completed', error_message = NULL, raw_response = ?4,
                 result_json = ?5, minutes_payload_json = ?6, diagnostics_json = ?7,
                 completed_at = ?8, updated_at = ?8
             WHERE id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'
               AND chunk_count = (
                 SELECT COUNT(*) FROM ai_summary_run_chunks
                 WHERE run_id = ?1 AND status = 'completed'
               )",
            params![
                lease.run_id,
                as_i64(lease.attempt_id, "attempt_id")?,
                as_i64(lease.lease_token, "lease_token")?,
                completion.raw_response,
                completion.result_json,
                completion.minutes_payload_json,
                completion.diagnostics_json,
                completion.completed_at,
            ],
        )
        .map_err(|error| error.to_string())?;
    if changed == 1 {
        transaction
            .execute(
                "UPDATE jobs
                 SET summary_status = 'completed', active_summary_run_id = ?1
                 WHERE id = ?2
                   AND EXISTS (
                     SELECT 1 FROM ai_summary_runs
                     WHERE id = ?1 AND job_id = ?2 AND status = 'completed'
                   )",
                params![lease.run_id, job_id],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(changed == 1)
}

pub fn fail_execution(
    conn: &mut Connection,
    lease: &AiSummaryRunLease,
    error_message: &str,
    now: &str,
) -> LocalResult<bool> {
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let job_id = transaction
        .query_row(
            "SELECT job_id FROM ai_summary_runs
             WHERE id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'",
            params![
                lease.run_id,
                as_i64(lease.attempt_id, "attempt_id")?,
                as_i64(lease.lease_token, "lease_token")?,
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some(job_id) = job_id else {
        return Ok(false);
    };
    let changed = transaction
        .execute(
            "UPDATE ai_summary_runs
             SET status = 'failed', error_message = ?4, updated_at = ?5
             WHERE id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'",
            params![
                lease.run_id,
                as_i64(lease.attempt_id, "attempt_id")?,
                as_i64(lease.lease_token, "lease_token")?,
                error_message,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    if changed == 1 {
        let summary_status = derive_job_summary_status(&transaction, &job_id, true)?;
        transaction
            .execute(
                "UPDATE jobs SET summary_status = ?2 WHERE id = ?1",
                params![job_id, summary_status],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(changed == 1)
}

pub fn delete_summary_run(conn: &mut Connection, job_id: &str, run_id: &str) -> LocalResult<()> {
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let active_run_id = transaction
        .query_row(
            "SELECT active_summary_run_id FROM jobs WHERE id = ?1",
            params![job_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .flatten();
    transaction
        .execute(
            "DELETE FROM ai_summary_runs WHERE job_id = ?1 AND id = ?2",
            params![job_id, run_id],
        )
        .map_err(|error| error.to_string())?;

    if active_run_id.as_deref() == Some(run_id) {
        let next_active = transaction
            .query_row(
                "SELECT id FROM ai_summary_runs
                 WHERE job_id = ?1 AND status = 'completed' AND result_json IS NOT NULL
                 ORDER BY completed_at DESC, updated_at DESC, id DESC
                 LIMIT 1",
                params![job_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let summary_status = derive_job_summary_status(&transaction, job_id, false)?;
        transaction
            .execute(
                "UPDATE jobs
                 SET active_summary_run_id = ?2, summary_status = ?3
                 WHERE id = ?1",
                params![job_id, next_active, summary_status],
            )
            .map_err(|error| error.to_string())?;
    } else {
        let summary_status = derive_job_summary_status(&transaction, job_id, false)?;
        transaction
            .execute(
                "UPDATE jobs SET summary_status = ?2 WHERE id = ?1",
                params![job_id, summary_status],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

pub fn delete_summary_runs_for_job_tx(
    transaction: &Transaction<'_>,
    job_id: &str,
) -> LocalResult<()> {
    transaction
        .execute(
            "DELETE FROM ai_summary_runs WHERE job_id = ?1",
            params![job_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn set_active_summary_run(conn: &Connection, job_id: &str, run_id: &str) -> LocalResult<()> {
    let changed = conn
        .execute(
            "UPDATE jobs
             SET active_summary_run_id = ?2, summary_status = 'completed'
             WHERE id = ?1 AND EXISTS (
               SELECT 1 FROM ai_summary_runs
               WHERE id = ?2 AND job_id = ?1 AND status = 'completed' AND result_json IS NOT NULL
             )",
            params![job_id, run_id],
        )
        .map_err(|error| error.to_string())?;
    if changed == 1 {
        Ok(())
    } else {
        Err("只能选择已完成且包含结果的 AI 总结运行。".into())
    }
}

fn map_summary_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiSummaryRun> {
    let result_json: Option<String> = row.get(11)?;
    let minutes_payload_json: Option<String> = row.get(12)?;
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
        result: result_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<AiSummaryResult>(value).ok()),
        minutes_payload: minutes_payload_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<MeetingMinutesPayload>(value).ok()),
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn derive_job_summary_status(
    transaction: &Transaction<'_>,
    job_id: &str,
    terminal_failure: bool,
) -> LocalResult<&'static str> {
    let (running, completed, failed) = transaction
        .query_row(
            "SELECT
               EXISTS(SELECT 1 FROM ai_summary_runs WHERE job_id = ?1 AND status = 'running'),
               EXISTS(SELECT 1 FROM ai_summary_runs WHERE job_id = ?1 AND status = 'completed' AND result_json IS NOT NULL),
               EXISTS(SELECT 1 FROM ai_summary_runs WHERE job_id = ?1 AND status = 'failed')",
            params![job_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? != 0,
                    row.get::<_, i64>(1)? != 0,
                    row.get::<_, i64>(2)? != 0,
                ))
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(if running {
        "summarizing"
    } else if completed {
        "completed"
    } else if terminal_failure || failed {
        "failed"
    } else {
        "idle"
    })
}

fn serialize_optional<T: serde::Serialize>(value: Option<&T>) -> LocalResult<Option<String>> {
    value
        .map(|value| serde_json::to_string(value).map_err(|error| error.to_string()))
        .transpose()
}

fn bool_to_sql(value: bool) -> i64 {
    i64::from(value)
}

fn empty_to_null(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then_some(value)
}

fn as_i64(value: u64, field: &str) -> LocalResult<i64> {
    i64::try_from(value).map_err(|_| format!("{field} 超出数据库范围。"))
}

fn to_u64(value: i64, field: &str) -> LocalResult<u64> {
    u64::try_from(value).map_err(|_| format!("数据库中的 {field} 无效。"))
}

fn to_usize(value: i64, field: &str) -> LocalResult<usize> {
    usize::try_from(value).map_err(|_| format!("数据库中的 {field} 无效。"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::repositories::ai_summary_run_models::AiSummaryChunkSeed;

    #[test]
    fn backfill_minutes_payload_fills_missing_once_without_overwriting_authority() {
        let mut connection = test_connection();
        insert_completed_run(
            &connection,
            "run-completed",
            "2026-07-15T01:00:00Z",
            "2026-07-15T01:00:00Z",
        );
        let payload = MeetingMinutesPayload {
            source_summary_run_id: Some("run-completed".into()),
            global_summary: vec!["first authority".into()],
            ..MeetingMinutesPayload::default()
        };

        assert!(
            backfill_minutes_payload(&mut connection, "job-1", "run-completed", &payload,)
                .expect("backfill missing payload")
        );

        let replacement = MeetingMinutesPayload {
            source_summary_run_id: Some("run-completed".into()),
            global_summary: vec!["must not replace".into()],
            ..MeetingMinutesPayload::default()
        };
        assert!(
            !backfill_minutes_payload(&mut connection, "job-1", "run-completed", &replacement,)
                .expect("preserve concurrent payload")
        );

        let stored = get_summary_run(&connection, "job-1", "run-completed")
            .expect("stored completed run")
            .minutes_payload
            .expect("stored minutes payload");
        assert_eq!(stored.global_summary, vec!["first authority".to_string()]);
    }

    #[test]
    fn stale_lease_cannot_publish_and_retry_only_loads_missing_chunks() {
        let mut connection = test_connection();
        let run = running_run("run-1", "job-1", "2026-07-15T01:00:00Z");
        let chunks = vec![
            AiSummaryChunkSeed {
                index: 0,
                sha256: "hash-0".into(),
                user_prompt: "chunk-0".into(),
            },
            AiSummaryChunkSeed {
                index: 1,
                sha256: "hash-1".into(),
                user_prompt: "chunk-1".into(),
            },
        ];
        let persisted = create_execution(
            &mut connection,
            &run,
            &NewAiSummaryExecution {
                transcript_revision: "revision-1",
                transcript_sha256: "transcript-hash",
                transcript_snapshot_json: "[]",
                execution_snapshot_json: "{}",
                chunks: &chunks,
            },
        )
        .expect("create execution");
        assert_eq!(persisted.id, "run-1");

        let first = claim_execution(&mut connection, "job-1", "run-1", "2026-07-15T01:01:00Z")
            .expect("claim first")
            .expect("first lease");
        let first_pending = list_pending_chunks(&connection, &first).expect("first pending chunks");
        assert_eq!(first_pending.len(), 2);
        assert!(save_chunk_result(
            &connection,
            &first,
            &first_pending[0],
            "raw-0",
            "{}",
            "2026-07-15T01:02:00Z",
        )
        .expect("save first chunk"));

        let second = claim_execution(&mut connection, "job-1", "run-1", "2026-07-15T01:03:00Z")
            .expect("claim second")
            .expect("second lease");
        assert_ne!(first, second);
        assert!(!fail_execution(
            &mut connection,
            &first,
            "stale failure",
            "2026-07-15T01:03:30Z",
        )
        .expect("reject stale failure"));
        assert_eq!(run_status(&connection, "run-1"), "running");
        assert_eq!(job_summary_status(&connection), "summarizing");
        assert!(!save_chunk_result(
            &connection,
            &first,
            &first_pending[1],
            "stale",
            "{}",
            "2026-07-15T01:04:00Z",
        )
        .expect("reject stale chunk"));
        let second_pending =
            list_pending_chunks(&connection, &second).expect("second pending chunks");
        assert_eq!(second_pending.len(), 1);
        assert_eq!(second_pending[0].index, 1);
        assert!(save_chunk_result(
            &connection,
            &second,
            &second_pending[0],
            "raw-1",
            "{}",
            "2026-07-15T01:05:00Z",
        )
        .expect("save remaining chunk"));
        assert_eq!(
            load_completed_chunk_responses(&connection, &second).expect("all responses"),
            vec!["raw-0", "raw-1"]
        );

        let completion = AiSummaryCompletion {
            raw_response: "merged",
            result_json: "{}",
            minutes_payload_json: "{}",
            diagnostics_json: "{}",
            completed_at: "2026-07-15T01:06:00Z",
        };
        assert!(!complete_execution(&mut connection, &first, &completion)
            .expect("reject stale completion"));
        assert!(complete_execution(&mut connection, &second, &completion)
            .expect("complete current lease"));
        assert!(!fail_execution(
            &mut connection,
            &first,
            "late stale failure",
            "2026-07-15T01:07:00Z",
        )
        .expect("reject stale failure after completion"));
        assert_eq!(run_status(&connection, "run-1"), "completed");
        assert_eq!(job_summary_status(&connection), "completed");
        assert_eq!(active_run_id(&connection), Some("run-1".into()));
    }

    #[test]
    fn current_failure_preserves_existing_completed_summary() {
        let mut connection = test_connection();
        insert_completed_run(
            &connection,
            "run-completed",
            "2026-07-15T00:30:00Z",
            "2026-07-15T00:30:00Z",
        );
        connection
            .execute(
                "UPDATE jobs
                 SET summary_status = 'completed', active_summary_run_id = 'run-completed'
                 WHERE id = 'job-1'",
                [],
            )
            .expect("select completed run");
        let chunks = vec![AiSummaryChunkSeed {
            index: 0,
            sha256: "hash-0".into(),
            user_prompt: "chunk-0".into(),
        }];
        create_execution(
            &mut connection,
            &running_run("run-retry", "job-1", "2026-07-15T01:00:00Z"),
            &NewAiSummaryExecution {
                transcript_revision: "revision-1",
                transcript_sha256: "hash-1",
                transcript_snapshot_json: "[]",
                execution_snapshot_json: "{}",
                chunks: &chunks,
            },
        )
        .expect("create retry");
        let lease = claim_execution(
            &mut connection,
            "job-1",
            "run-retry",
            "2026-07-15T01:01:00Z",
        )
        .expect("claim retry")
        .expect("retry lease");

        assert!(fail_execution(
            &mut connection,
            &lease,
            "request failed",
            "2026-07-15T01:02:00Z",
        )
        .expect("fail retry"));
        assert_eq!(run_status(&connection, "run-retry"), "failed");
        assert_eq!(job_summary_status(&connection), "completed");
        assert_eq!(active_run_id(&connection), Some("run-completed".into()));
    }

    #[test]
    fn double_create_returns_existing_running_execution() {
        let mut connection = test_connection();
        let chunks = vec![AiSummaryChunkSeed {
            index: 0,
            sha256: "hash-0".into(),
            user_prompt: "chunk-0".into(),
        }];
        let first = create_execution(
            &mut connection,
            &running_run("run-first", "job-1", "2026-07-15T01:00:00Z"),
            &NewAiSummaryExecution {
                transcript_revision: "revision-1",
                transcript_sha256: "hash-1",
                transcript_snapshot_json: "[]",
                execution_snapshot_json: "{}",
                chunks: &chunks,
            },
        )
        .expect("first create");
        let second = create_execution(
            &mut connection,
            &running_run("run-second", "job-1", "2026-07-15T01:01:00Z"),
            &NewAiSummaryExecution {
                transcript_revision: "revision-2",
                transcript_sha256: "hash-2",
                transcript_snapshot_json: "[]",
                execution_snapshot_json: "{}",
                chunks: &chunks,
            },
        )
        .expect("second create");

        assert_eq!(first.id, "run-first");
        assert_eq!(second.id, "run-first");
        let running_count = connection
            .query_row(
                "SELECT COUNT(*) FROM ai_summary_runs WHERE job_id = 'job-1' AND status = 'running'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("running count");
        let chunk_count = connection
            .query_row(
                "SELECT COUNT(*) FROM ai_summary_run_chunks WHERE run_id = 'run-first'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("chunk count");
        assert_eq!(running_count, 1);
        assert_eq!(chunk_count, 1);
    }

    #[test]
    fn migration_collapses_duplicate_running_runs_and_enforces_unique_index() {
        let mut connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE jobs (
                   id TEXT PRIMARY KEY,
                   summary_status TEXT NOT NULL,
                   active_summary_run_id TEXT
                 );
                 CREATE TABLE ai_summary_runs (
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
                   minutes_payload_json TEXT,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   execution_snapshot_json TEXT NOT NULL DEFAULT '',
                   chunk_count INTEGER NOT NULL DEFAULT 0,
                   FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
                 );
                 INSERT INTO jobs VALUES ('job-1', 'summarizing', NULL);
                 INSERT INTO ai_summary_runs (
                   id, job_id, status, created_at, updated_at,
                   execution_snapshot_json, chunk_count
                 ) VALUES
                   ('run-old', 'job-1', 'running', '2026-07-15T00:00:00Z',
                    '2026-07-15T01:00:00Z', '{}', 1),
                   ('run-new', 'job-1', 'running', '2026-07-15T00:00:00Z',
                    '2026-07-15T02:00:00Z', '{}', 1),
                   ('run-newer-legacy', 'job-1', 'running', '2026-07-15T00:00:00Z',
                    '2026-07-15T03:00:00Z', '', 0);",
            )
            .expect("duplicate running fixture");
        let transaction = connection.transaction().expect("migration transaction");
        migrate_v7_ai_summary_runs(&transaction).expect("v7 migration");
        transaction.commit().expect("commit migration");

        assert_eq!(run_status(&connection, "run-old"), "failed");
        assert_eq!(run_status(&connection, "run-new"), "running");
        assert_eq!(run_status(&connection, "run-newer-legacy"), "failed");
        assert_eq!(job_summary_status(&connection), "summarizing");
        let duplicate = connection.execute(
            "INSERT INTO ai_summary_runs (
               id, job_id, status, created_at, updated_at,
               execution_snapshot_json, chunk_count
             ) VALUES (
               'run-third', 'job-1', 'running', '2026-07-15T03:00:00Z',
               '2026-07-15T03:00:00Z', '{}', 1
             )",
            [],
        );
        assert!(
            duplicate.is_err(),
            "partial unique index accepted a second running run"
        );
    }

    #[test]
    fn deleting_non_active_preserves_active_and_active_delete_selects_latest_completed() {
        let mut connection = test_connection();
        insert_completed_run(
            &connection,
            "run-active",
            "2026-07-15T02:00:00Z",
            "2026-07-15T03:00:00Z",
        );
        insert_completed_run(
            &connection,
            "run-old",
            "2026-07-15T01:00:00Z",
            "2026-07-15T01:00:00Z",
        );
        insert_completed_run(
            &connection,
            "run-next-a",
            "2026-07-15T02:30:00Z",
            "2026-07-15T02:30:00Z",
        );
        insert_completed_run(
            &connection,
            "run-next-b",
            "2026-07-15T02:30:00Z",
            "2026-07-15T02:30:00Z",
        );
        connection
            .execute(
                "UPDATE jobs
                 SET active_summary_run_id = 'run-active', summary_status = 'summarizing'
                 WHERE id = 'job-1'",
                [],
            )
            .expect("select active");
        connection
            .execute(
                "INSERT INTO ai_summary_runs (
                   id, job_id, status, created_at, updated_at,
                   execution_snapshot_json, chunk_count
                 ) VALUES (
                   'run-running', 'job-1', 'running', '2026-07-15T04:00:00Z',
                   '2026-07-15T04:00:00Z', '{}', 1
                 )",
                [],
            )
            .expect("insert running run");

        delete_summary_run(&mut connection, "job-1", "run-old").expect("delete non-active run");
        assert_eq!(active_run_id(&connection), Some("run-active".into()));

        delete_summary_run(&mut connection, "job-1", "run-active").expect("delete active run");
        assert_eq!(active_run_id(&connection), Some("run-next-b".into()));
        assert_eq!(job_summary_status(&connection), "summarizing");

        delete_summary_run(&mut connection, "job-1", "run-next-b").expect("delete replacement");
        delete_summary_run(&mut connection, "job-1", "run-next-a").expect("delete last completed");
        assert_eq!(active_run_id(&connection), None);
        assert_eq!(job_summary_status(&connection), "summarizing");
        delete_summary_run(&mut connection, "job-1", "run-running").expect("delete running run");
        assert_eq!(job_summary_status(&connection), "idle");
    }

    fn test_connection() -> Connection {
        let mut connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE jobs (
                   id TEXT PRIMARY KEY,
                   summary_status TEXT NOT NULL,
                   active_summary_run_id TEXT
                 );
                 CREATE TABLE ai_summary_runs (
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
                   minutes_payload_json TEXT,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
                 );
                 INSERT INTO jobs VALUES ('job-1', 'idle', NULL);",
            )
            .expect("base schema");
        let transaction = connection.transaction().expect("migration transaction");
        migrate_v7_ai_summary_runs(&transaction).expect("v7 schema");
        transaction.commit().expect("commit schema");
        connection
    }

    fn running_run(id: &str, job_id: &str, now: &str) -> AiSummaryRun {
        AiSummaryRun {
            id: id.into(),
            job_id: job_id.into(),
            model_config_id: "model-1".into(),
            template_id: "template-1".into(),
            include_speaker: true,
            include_timestamp: true,
            extra_instructions: String::new(),
            status: "running".into(),
            error_message: None,
            prompt_preview: Some("prompt".into()),
            raw_response: None,
            result: None,
            minutes_payload: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    fn insert_completed_run(
        connection: &Connection,
        id: &str,
        updated_at: &str,
        completed_at: &str,
    ) {
        connection
            .execute(
                "INSERT INTO ai_summary_runs (
                   id, job_id, status, result_json, created_at, updated_at, completed_at
                 ) VALUES (?1, 'job-1', 'completed', '{}', ?2, ?2, ?3)",
                params![id, updated_at, completed_at],
            )
            .expect("insert completed run");
    }

    fn active_run_id(connection: &Connection) -> Option<String> {
        connection
            .query_row(
                "SELECT active_summary_run_id FROM jobs WHERE id = 'job-1'",
                [],
                |row| row.get(0),
            )
            .expect("active run")
    }

    fn run_status(connection: &Connection, run_id: &str) -> String {
        connection
            .query_row(
                "SELECT status FROM ai_summary_runs WHERE id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .expect("run status")
    }

    fn job_summary_status(connection: &Connection) -> String {
        connection
            .query_row(
                "SELECT summary_status FROM jobs WHERE id = 'job-1'",
                [],
                |row| row.get(0),
            )
            .expect("job summary status")
    }
}
