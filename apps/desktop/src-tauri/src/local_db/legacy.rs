use crate::{
    infrastructure::repositories::ai_summary_runs,
    local_db::{jobs, jobs_root, LocalResult},
};
use rusqlite::{Connection, OptionalExtension};
use std::{fs, path::Path};
use tauri::AppHandle;

use super::{model::*, progress};

pub(crate) fn import_legacy_jobs(app: &AppHandle, conn: &mut Connection) -> LocalResult<()> {
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
            jobs::save_job_snapshot_tx(&tx, &job)?;

            if has_summary_content(&job.summary) {
                let imported_run = imported_summary_run(&job);
                ai_summary_runs::save_summary_run_tx(&tx, &imported_run)?;
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

    if let Ok(progress) = progress::read_json::<ProgressSnapshot>(&job_dir.join("progress.json")) {
        progress::apply_progress_snapshot(&mut job, &progress);
    }

    if let Ok(result) = progress::read_json::<LegacyRunnerResult>(&job_dir.join("result.json")) {
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
        minutes_payload: None,
        created_at: job.created_at.clone(),
        updated_at: job.created_at.clone(),
    }
}
