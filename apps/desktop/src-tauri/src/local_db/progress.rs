use crate::{infrastructure::runner_protocol, local_db::LocalResult};
use serde::Deserialize;
use std::{fs, path::Path};

use super::model::{MeetingJob, ProgressSnapshot};

pub(crate) fn apply_progress_snapshot(job: &mut MeetingJob, progress: &ProgressSnapshot) {
    job.progress_percent = progress.progress_percent;
    job.progress_message = progress
        .status_message
        .clone()
        .filter(|value| !value.trim().is_empty());

    match progress.stage.as_str() {
        "queued" => {
            job.upload_status = "uploaded".into();
            job.asr_status = "queued".into();
            job.summary_status = "idle".into();
            job.overall_status = "queued".into();
            job.progress_percent = Some(job.progress_percent.unwrap_or(0));
        }
        "transcribing" => {
            job.upload_status = "uploaded".into();
            job.asr_status = "transcribing".into();
            job.summary_status = "idle".into();
            job.overall_status = "transcribing".into();
            job.progress_percent = Some(job.progress_percent.unwrap_or(12));
        }
        "speaker_processing" => {
            job.upload_status = "uploaded".into();
            job.asr_status = "speaker_processing".into();
            job.summary_status = "idle".into();
            job.overall_status = "speaker_processing".into();
            job.progress_percent = Some(job.progress_percent.unwrap_or(92));
        }
        "completed" => {
            job.upload_status = "uploaded".into();
            job.asr_status = "completed".into();
            if job.summary_status == "queued" {
                job.summary_status = "idle".into();
            }
            job.overall_status = "completed".into();
            job.progress_percent = Some(100);
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

pub(crate) fn read_runner_progress(path: &Path) -> LocalResult<ProgressSnapshot> {
    let progress = runner_protocol::read_progress(path)?;
    Ok(ProgressSnapshot {
        stage: progress.stage,
        status_message: progress.status_message,
        failure_reason: progress.failure_reason,
        progress_percent: progress.progress_percent,
    })
}

pub(crate) fn read_legacy_json<T: for<'de> Deserialize<'de>>(path: &Path) -> LocalResult<T> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}
