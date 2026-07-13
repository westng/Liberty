use crate::local_db::{
    AiSummaryResult, AiSummaryRun, LocalResult, MeetingJob, MeetingMinutesPayload,
};

#[derive(Debug, Clone)]
pub struct SummaryExportSource {
    pub result: AiSummaryResult,
    pub cached_projection: Option<MeetingMinutesPayload>,
    pub template_id: String,
    pub run_id: Option<String>,
}

pub fn resolve_summary_source(
    job: &MeetingJob,
    requested_run_id: Option<&str>,
) -> LocalResult<SummaryExportSource> {
    if let Some(run_id) = requested_run_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let run = job
            .summary_runs
            .iter()
            .find(|run| run.id == run_id)
            .ok_or_else(|| format!("导出失败：没有找到指定的 AI 总结版本 {run_id}。"))?;
        return summary_source_from_run(run);
    }

    if let Some(active_run_id) = job.active_summary_run_id.as_deref() {
        let run = job
            .summary_runs
            .iter()
            .find(|run| run.id == active_run_id)
            .ok_or_else(|| "导出失败：当前活动的 AI 总结版本不存在。".to_string())?;
        return summary_source_from_run(run);
    }

    if has_legacy_summary(job) {
        return Ok(SummaryExportSource {
            result: AiSummaryResult {
                title: job.title.clone(),
                overview: job.summary.overview.clone(),
                topics: job.summary.topics.clone(),
                decisions: Vec::new(),
                action_items: Vec::new(),
                risks: Vec::new(),
                follow_ups: Vec::new(),
            },
            cached_projection: None,
            template_id: String::new(),
            run_id: None,
        });
    }

    Err("导出失败：当前任务没有可导出的 AI 总结。".into())
}

fn summary_source_from_run(run: &AiSummaryRun) -> LocalResult<SummaryExportSource> {
    if run.status != "completed" {
        return Err(format!("导出失败：AI 总结版本 {} 尚未完成。", run.id));
    }

    let result = run
        .result
        .clone()
        .ok_or_else(|| format!("导出失败：AI 总结版本 {} 没有结果数据。", run.id))?;

    Ok(SummaryExportSource {
        result,
        cached_projection: run.minutes_payload.clone(),
        template_id: run.template_id.clone(),
        run_id: Some(run.id.clone()),
    })
}

fn has_legacy_summary(job: &MeetingJob) -> bool {
    !job.summary.overview.trim().is_empty()
        || !job.summary.topics.is_empty()
        || !job.summary.decisions.is_empty()
        || !job.summary.action_items.is_empty()
        || !job.summary.risks.is_empty()
        || !job.summary.follow_ups.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completed_run(id: &str, title: &str) -> AiSummaryRun {
        AiSummaryRun {
            id: id.into(),
            job_id: "job-1".into(),
            status: "completed".into(),
            result: Some(AiSummaryResult {
                title: title.into(),
                overview: format!("会议名称：{title}"),
                ..AiSummaryResult::default()
            }),
            ..AiSummaryRun::default()
        }
    }

    #[test]
    fn requested_run_is_the_export_authority() {
        let job = MeetingJob {
            active_summary_run_id: Some("run-active".into()),
            summary_runs: vec![
                completed_run("run-active", "活动版本"),
                completed_run("run-requested", "指定版本"),
            ],
            ..MeetingJob::default()
        };

        let source = resolve_summary_source(&job, Some("run-requested")).unwrap();

        assert_eq!(source.run_id.as_deref(), Some("run-requested"));
        assert_eq!(source.result.title, "指定版本");
    }

    #[test]
    fn unresolved_requested_run_does_not_fall_back_to_another_version() {
        let job = MeetingJob {
            active_summary_run_id: Some("run-active".into()),
            summary_runs: vec![completed_run("run-active", "活动版本")],
            ..MeetingJob::default()
        };

        let error = resolve_summary_source(&job, Some("run-missing")).unwrap_err();

        assert!(error.contains("run-missing"));
    }

    #[test]
    fn active_run_is_used_when_no_version_is_requested() {
        let job = MeetingJob {
            active_summary_run_id: Some("run-active".into()),
            summary_runs: vec![completed_run("run-active", "活动版本")],
            ..MeetingJob::default()
        };

        let source = resolve_summary_source(&job, None).unwrap();

        assert_eq!(source.run_id.as_deref(), Some("run-active"));
    }
}
