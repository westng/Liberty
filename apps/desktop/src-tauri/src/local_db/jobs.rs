use crate::{
    infrastructure::repositories::ai_summary_runs,
    local_db::{job_dir, LocalResult},
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::fs;
use tauri::AppHandle;

use super::{model::*, progress};

pub(crate) fn save_job_snapshot_tx(tx: &Transaction<'_>, job: &MeetingJob) -> LocalResult<()> {
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
                    progress_percent: None,
                    progress_message: None,
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

    let dir = job_dir(app, &job.id)?;
    if should_apply_progress_snapshot(&job) {
        if let Ok(progress) = progress::read_json::<ProgressSnapshot>(&dir.join("progress.json")) {
            progress::apply_progress_snapshot(&mut job, &progress);
        }
    }

    job.process_log = fs::read_to_string(dir.join("process.log"))
        .ok()
        .map(|content| content.trim_end().to_string())
        .filter(|content| !content.is_empty());

    Ok(job)
}

fn should_apply_progress_snapshot(job: &MeetingJob) -> bool {
    !matches!(job.overall_status.as_str(), "completed" | "failed")
}

pub(crate) fn load_job_summary(
    app: &AppHandle,
    conn: &Connection,
    job_id: &str,
) -> LocalResult<MeetingJob> {
    let mut job = conn
        .query_row(
            "SELECT id, title, created_at, duration_minutes, lang, enable_speaker,
                    summary_template, upload_status, asr_status, summary_status,
                    overall_status, processing_started_at_ms, processing_finished_at_ms,
                    processing_duration_seconds, failure_reason, active_summary_run_id,
                    last_exported_at, hotwords_json, export_formats_json
             FROM jobs WHERE id = ?1",
            params![job_id],
            |row| {
                Ok(MeetingJob {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    duration_minutes: row.get::<_, i64>(3)? as u32,
                    created_at: row.get(2)?,
                    processing_started_at_ms: row
                        .get::<_, Option<i64>>(11)?
                        .map(|value| value as u64),
                    processing_finished_at_ms: row
                        .get::<_, Option<i64>>(12)?
                        .map(|value| value as u64),
                    processing_duration_seconds: row
                        .get::<_, Option<i64>>(13)?
                        .map(|value| value as u32),
                    progress_percent: None,
                    progress_message: None,
                    hotwords: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(17)?)
                        .unwrap_or_default(),
                    lang: row.get(4)?,
                    enable_speaker: row.get::<_, i64>(5)? != 0,
                    summary_template: row.get(6)?,
                    upload_status: row.get(7)?,
                    asr_status: row.get(8)?,
                    summary_status: row.get(9)?,
                    overall_status: row.get(10)?,
                    failure_reason: row.get(14)?,
                    active_summary_run_id: row.get(15)?,
                    last_exported_at: row.get(16)?,
                    export_formats: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(18)?)
                        .unwrap_or_else(|_| {
                            vec!["txt".into(), "md".into(), "srt".into(), "docx".into()]
                        }),
                    ..MeetingJob::default()
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "没有找到这个任务。".to_string())?;

    job.source_files = load_source_files(conn, &job.id)?;
    let dir = job_dir(app, &job.id)?;
    if should_apply_progress_snapshot(&job) {
        if let Ok(progress) = progress::read_json::<ProgressSnapshot>(&dir.join("progress.json")) {
            progress::apply_progress_snapshot(&mut job, &progress);
        }
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
