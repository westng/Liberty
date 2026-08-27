use crate::{
    domain::transcript::TranscriptSegment,
    infrastructure::{repositories::ai_summary_runs, runner_files},
    local_db::{jobs_root, LocalResult},
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tauri::AppHandle;

use super::{model::*, progress};

pub(crate) fn save_job_snapshot_tx(tx: &Transaction<'_>, job: &MeetingJob) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO jobs (
            id, title, created_at, duration_minutes, lang, enable_speaker,
            runner_protocol_version, asr_backend, diarization_status, warnings_json,
            summary_template, upload_status, asr_status, summary_status, overall_status,
            processing_started_at_ms, processing_finished_at_ms, processing_duration_seconds,
            failure_reason, process_log, python_path, runner_script_path, active_summary_run_id,
            last_exported_at, hotwords_json, export_formats_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            created_at = excluded.created_at,
            duration_minutes = excluded.duration_minutes,
            lang = excluded.lang,
            enable_speaker = excluded.enable_speaker,
            runner_protocol_version = excluded.runner_protocol_version,
            asr_backend = excluded.asr_backend,
            diarization_status = excluded.diarization_status,
            warnings_json = excluded.warnings_json,
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
            job.runner_protocol_version.map(i64::from),
            job.asr_backend.as_str(),
            job.diarization_status.as_str(),
            serde_json::to_string(&job.warnings).map_err(|err| err.to_string())?,
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

    tx.execute(
        "DELETE FROM job_source_files WHERE job_id = ?1",
        params![job.id],
    )
    .map_err(|err| err.to_string())?;

    for file in &job.source_files {
        tx.execute(
            "INSERT INTO job_source_files (id, job_id, name, path, size_label, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                file.id,
                job.id,
                file.name,
                file.path,
                file.size_label,
                file.kind
            ],
        )
        .map_err(|err| err.to_string())?;
    }

    replace_segments_tx(tx, &job.id, "transcript", &job.transcript_segments)?;
    replace_segments_tx(tx, &job.id, "speaker", &job.speaker_segments)?;

    Ok(())
}

pub(crate) fn replace_segments_tx(
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

pub(crate) fn load_job(
    app: &AppHandle,
    conn: &Connection,
    job_id: &str,
) -> LocalResult<MeetingJob> {
    let base = load_job_base(conn, job_id)?;

    let mut job = base.ok_or_else(|| "没有找到这个任务。".to_string())?;
    job.source_files = load_source_files(conn, &job.id)?;
    job.transcript_segments = load_segments(conn, &job.id, "transcript")?;
    job.speaker_segments = load_segments(conn, &job.id, "speaker")?;
    if job.duration_minutes == 0 {
        job.duration_minutes =
            derive_duration_minutes_from_segments(&job.transcript_segments, &job.speaker_segments)
                .unwrap_or(0);
    }
    job.summary_runs = ai_summary_runs::list_summary_runs(conn, &job.id)?;
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

    if should_apply_progress_snapshot(&job) {
        if let Some(dir) = active_attempt_dir(app, conn, &job.id)? {
            if let Ok(progress) = progress::read_runner_progress(&dir.join("progress.json")) {
                progress::apply_progress_snapshot(&mut job, &progress);
            }
        }
    }

    Ok(job)
}

fn load_job_base(conn: &Connection, job_id: &str) -> LocalResult<Option<MeetingJob>> {
    conn
        .query_row(
            "SELECT id, title, created_at, duration_minutes, lang, enable_speaker,
                    runner_protocol_version, asr_backend, diarization_status, warnings_json,
                    summary_template, upload_status, asr_status, summary_status,
                    overall_status, processing_started_at_ms, processing_finished_at_ms,
                    processing_duration_seconds, failure_reason, process_log, python_path,
                    runner_script_path, active_summary_run_id, last_exported_at, hotwords_json, export_formats_json
             FROM jobs WHERE id = ?1",
            params![job_id],
            |row| {
                Ok(MeetingJob {
                    id: row.get(0)?,
                    source: "local".into(),
                    title: row.get(1)?,
                    duration_minutes: row.get::<_, i64>(3)? as u32,
                    created_at: row.get(2)?,
                    processing_started_at_ms: row.get::<_, Option<i64>>(15)?.map(|value| value as u64),
                    processing_finished_at_ms: row.get::<_, Option<i64>>(16)?.map(|value| value as u64),
                    processing_duration_seconds: row.get::<_, Option<i64>>(17)?.map(|value| value as u32),
                    progress_percent: None,
                    progress_message: None,
                    hotwords: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(24)?)
                        .unwrap_or_default(),
                    lang: row.get(4)?,
                    enable_speaker: row.get::<_, i64>(5)? != 0,
                    runner_protocol_version: row.get::<_, Option<i64>>(6)?.map(|value| value as u32),
                    asr_backend: serde_json::from_value(serde_json::Value::String(row.get(7)?))
                        .unwrap_or_default(),
                    diarization_status: serde_json::from_value(serde_json::Value::String(row.get(8)?))
                        .unwrap_or_default(),
                    warnings: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
                    summary_template: row.get(10)?,
                    upload_status: row.get(11)?,
                    asr_status: row.get(12)?,
                    summary_status: row.get(13)?,
                    overall_status: row.get(14)?,
                    failure_reason: row.get(18)?,
                    process_log: row.get(19)?,
                    python_path: row.get(20)?,
                    runner_script_path: row.get(21)?,
                    active_summary_run_id: row.get(22)?,
                    last_exported_at: row.get(23)?,
                    export_formats: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(25)?)
                        .unwrap_or_else(|_| vec!["txt".into(), "md".into(), "srt".into(), "docx".into()]),
                    ..MeetingJob::default()
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())
}

fn should_apply_progress_snapshot(job: &MeetingJob) -> bool {
    !matches!(job.overall_status.as_str(), "completed" | "failed")
}

pub(crate) fn list_job_summaries(
    app: &AppHandle,
    conn: &Connection,
) -> LocalResult<Vec<MeetingJob>> {
    let mut jobs = query_job_summaries(conn)?;
    if jobs.is_empty() {
        return Ok(jobs);
    }

    attach_source_files(conn, &mut jobs)?;
    let attempt_dirs = active_attempt_dirs(app, conn)?;
    apply_active_progress_snapshots(&mut jobs, &attempt_dirs, progress::read_runner_progress);
    Ok(jobs)
}

fn query_job_summaries(conn: &Connection) -> LocalResult<Vec<MeetingJob>> {
    let mut statement = conn
        .prepare(
            "SELECT id, title, created_at, duration_minutes, lang, enable_speaker,
                    runner_protocol_version, asr_backend, diarization_status, warnings_json,
                    summary_template, upload_status, asr_status, summary_status,
                    overall_status, processing_started_at_ms, processing_finished_at_ms,
                    processing_duration_seconds, failure_reason, active_summary_run_id,
                    last_exported_at, hotwords_json, export_formats_json
             FROM jobs
             WHERE NOT EXISTS (
               SELECT 1 FROM job_deletion_ops WHERE job_deletion_ops.job_id = jobs.id
             )
             ORDER BY created_at DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], map_job_summary_row)
        .map_err(|error| error.to_string())?;
    let jobs = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(jobs)
}

fn attach_source_files(conn: &Connection, jobs: &mut [MeetingJob]) -> LocalResult<()> {
    let job_indexes = jobs
        .iter()
        .enumerate()
        .map(|(index, job)| (job.id.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut source_statement = conn
        .prepare(
            "SELECT source.job_id, source.id, source.name, source.path,
                    source.size_label, source.kind
             FROM job_source_files source
             INNER JOIN jobs ON jobs.id = source.job_id
             WHERE NOT EXISTS (
               SELECT 1 FROM job_deletion_ops WHERE job_deletion_ops.job_id = jobs.id
             )
             ORDER BY source.rowid ASC",
        )
        .map_err(|error| error.to_string())?;
    let source_rows = source_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                MeetingSourceFile {
                    id: row.get(1)?,
                    name: row.get(2)?,
                    path: row.get(3)?,
                    size_label: row.get(4)?,
                    kind: row.get(5)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    for source in source_rows {
        let (job_id, source) = source.map_err(|error| error.to_string())?;
        if let Some(index) = job_indexes.get(&job_id) {
            jobs[*index].source_files.push(source);
        }
    }
    Ok(())
}

fn apply_active_progress_snapshots(
    jobs: &mut [MeetingJob],
    attempt_dirs: &HashMap<String, PathBuf>,
    mut read_progress: impl FnMut(&Path) -> LocalResult<ProgressSnapshot>,
) {
    for job in jobs {
        if should_apply_progress_snapshot(job) {
            if let Some(dir) = attempt_dirs.get(&job.id) {
                if let Ok(progress) = read_progress(&dir.join("progress.json")) {
                    progress::apply_progress_snapshot(job, &progress);
                }
            }
        }
    }
}

fn map_job_summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MeetingJob> {
    Ok(MeetingJob {
        id: row.get(0)?,
        source: "local".into(),
        title: row.get(1)?,
        duration_minutes: row.get::<_, i64>(3)? as u32,
        created_at: row.get(2)?,
        processing_started_at_ms: row.get::<_, Option<i64>>(15)?.map(|value| value as u64),
        processing_finished_at_ms: row.get::<_, Option<i64>>(16)?.map(|value| value as u64),
        processing_duration_seconds: row.get::<_, Option<i64>>(17)?.map(|value| value as u32),
        progress_percent: None,
        progress_message: None,
        hotwords: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(21)?)
            .unwrap_or_default(),
        lang: row.get(4)?,
        enable_speaker: row.get::<_, i64>(5)? != 0,
        runner_protocol_version: row.get::<_, Option<i64>>(6)?.map(|value| value as u32),
        asr_backend: serde_json::from_value(serde_json::Value::String(row.get(7)?))
            .unwrap_or_default(),
        diarization_status: serde_json::from_value(serde_json::Value::String(row.get(8)?))
            .unwrap_or_default(),
        warnings: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
        summary_template: row.get(10)?,
        upload_status: row.get(11)?,
        asr_status: row.get(12)?,
        summary_status: row.get(13)?,
        overall_status: row.get(14)?,
        failure_reason: row.get(18)?,
        active_summary_run_id: row.get(19)?,
        last_exported_at: row.get(20)?,
        export_formats: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(22)?)
            .unwrap_or_else(|_| vec!["txt".into(), "md".into(), "srt".into(), "docx".into()]),
        ..MeetingJob::default()
    })
}

fn active_attempt_dirs(
    app: &AppHandle,
    conn: &Connection,
) -> LocalResult<HashMap<String, PathBuf>> {
    let root = jobs_root(app)?;
    let mut statement = conn
        .prepare(
            "SELECT runs.job_id, runs.attempt_id, runs.lease_token, runs.output_dir
             FROM job_runs runs
             INNER JOIN jobs ON jobs.id = runs.job_id
             WHERE runs.status = 'running'
               AND NOT EXISTS (
                 SELECT 1 FROM job_deletion_ops WHERE job_deletion_ops.job_id = jobs.id
               )",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut dirs = HashMap::new();
    for row in rows {
        let (job_id, attempt_id, lease_token, output_dir) =
            row.map_err(|error| error.to_string())?;
        if attempt_id < 0 || lease_token < 0 {
            return Err("任务运行记录包含无效的 attempt 或 lease。".into());
        }
        let expected_output_dir = format!("attempts/attempt-{attempt_id}-{lease_token}");
        if output_dir.as_deref() != Some(expected_output_dir.as_str()) {
            return Err("任务运行记录的输出目录与当前 lease 不一致。".into());
        }
        let job_dir = runner_files::resolve_job_dir(&root, &job_id)?;
        if let Some(dir) =
            runner_files::resolve_attempt_dir(&job_dir, attempt_id as u64, lease_token as u64)?
        {
            dirs.insert(job_id, dir);
        }
    }
    Ok(dirs)
}

fn active_attempt_dir(
    app: &AppHandle,
    conn: &Connection,
    job_id: &str,
) -> LocalResult<Option<PathBuf>> {
    let run = conn
        .query_row(
            "SELECT attempt_id, lease_token, output_dir
             FROM job_runs
             WHERE job_id = ?1 AND status = 'running'",
            params![job_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|err| err.to_string())?;
    let Some((attempt_id, lease_token, output_dir)) = run else {
        return Ok(None);
    };
    if attempt_id < 0 || lease_token < 0 {
        return Err("任务运行记录包含无效的 attempt 或 lease。".into());
    }

    let expected_output_dir = format!("attempts/attempt-{attempt_id}-{lease_token}");
    if output_dir.as_deref() != Some(expected_output_dir.as_str()) {
        return Err("任务运行记录的输出目录与当前 lease 不一致。".into());
    }

    let job_dir = runner_files::resolve_job_dir(&jobs_root(app)?, job_id)?;
    runner_files::resolve_attempt_dir(&job_dir, attempt_id as u64, lease_token as u64)
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

fn load_segments(
    conn: &Connection,
    job_id: &str,
    segment_type: &str,
) -> LocalResult<Vec<TranscriptSegment>> {
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

fn segment_row_id(job_id: &str, segment_type: &str, segment_id: &str) -> String {
    format!("{job_id}:{segment_type}:{segment_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("database");
        crate::local_db::schema::apply_test_schema(&connection).expect("schema");
        connection
    }

    fn job(id: &str, status: &str, source_files: Vec<MeetingSourceFile>) -> MeetingJob {
        MeetingJob {
            id: id.into(),
            source: "local".into(),
            title: id.into(),
            source_files,
            created_at: format!("2026-08-13T0{}:00:00.000Z", id.len()),
            lang: "zh".into(),
            summary_template: String::new(),
            upload_status: "uploaded".into(),
            asr_status: status.into(),
            summary_status: "idle".into(),
            overall_status: status.into(),
            export_formats: vec!["txt".into()],
            ..MeetingJob::default()
        }
    }

    #[test]
    fn full_job_query_preserves_local_source() {
        let mut connection = test_connection();
        let transaction = connection.transaction().expect("transaction");
        save_job_snapshot_tx(&transaction, &job("job-local", "completed", Vec::new()))
            .expect("job");
        transaction.commit().expect("commit");

        let loaded = load_job_base(&connection, "job-local")
            .expect("load job")
            .expect("stored job");

        assert_eq!(loaded.source, "local");
    }

    #[test]
    fn summary_query_attaches_all_sources_without_loading_details() {
        let mut connection = test_connection();
        let transaction = connection.transaction().expect("transaction");
        save_job_snapshot_tx(
            &transaction,
            &job(
                "job-a",
                "completed",
                vec![
                    MeetingSourceFile {
                        id: "source-a1".into(),
                        name: "first.wav".into(),
                        kind: "audio".into(),
                        ..MeetingSourceFile::default()
                    },
                    MeetingSourceFile {
                        id: "source-a2".into(),
                        name: "second.wav".into(),
                        kind: "audio".into(),
                        ..MeetingSourceFile::default()
                    },
                ],
            ),
        )
        .expect("first job");
        save_job_snapshot_tx(
            &transaction,
            &job(
                "job-b",
                "queued",
                vec![MeetingSourceFile {
                    id: "source-b1".into(),
                    name: "third.mp4".into(),
                    kind: "video".into(),
                    ..MeetingSourceFile::default()
                }],
            ),
        )
        .expect("second job");
        transaction.commit().expect("commit");

        let mut jobs = query_job_summaries(&connection).expect("summaries");
        attach_source_files(&connection, &mut jobs).expect("sources");

        let sources_by_job = jobs
            .iter()
            .map(|job| {
                (
                    job.id.as_str(),
                    job.source_files
                        .iter()
                        .map(|source| source.name.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        assert_eq!(sources_by_job["job-a"], vec!["first.wav", "second.wav"]);
        assert_eq!(sources_by_job["job-b"], vec!["third.mp4"]);
        assert!(jobs.iter().all(|job| job.source == "local"));
        assert!(jobs.iter().all(|job| job.transcript_segments.is_empty()));
        assert!(jobs.iter().all(|job| job.summary_runs.is_empty()));
    }

    #[test]
    fn progress_snapshots_are_read_only_for_active_jobs() {
        let mut jobs = vec![
            job("active", "queued", Vec::new()),
            job("complete", "completed", Vec::new()),
            job("failed", "failed", Vec::new()),
        ];
        let attempt_dirs = jobs
            .iter()
            .map(|job| (job.id.clone(), PathBuf::from(&job.id)))
            .collect::<HashMap<_, _>>();
        let mut requested_paths = Vec::new();

        apply_active_progress_snapshots(&mut jobs, &attempt_dirs, |path| {
            requested_paths.push(path.to_path_buf());
            Ok(ProgressSnapshot {
                stage: "transcribing".into(),
                status_message: Some("working".into()),
                failure_reason: None,
                progress_percent: Some(40),
            })
        });

        assert_eq!(requested_paths, vec![PathBuf::from("active/progress.json")]);
        assert_eq!(jobs[0].overall_status, "transcribing");
        assert_eq!(jobs[0].progress_percent, Some(40));
        assert_eq!(jobs[1].overall_status, "completed");
        assert_eq!(jobs[2].overall_status, "failed");
    }
}
