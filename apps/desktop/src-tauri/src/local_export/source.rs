use std::collections::HashSet;

use crate::local_db::{
    AiSummaryActionItem, AiSummaryResult, AiSummaryRun, LocalResult, MeetingJob, MeetingMember,
    MeetingMinutesPayload,
};

#[derive(Debug, Clone)]
pub struct SummaryExportSource {
    pub result: AiSummaryResult,
    pub minutes_payload: MeetingMinutesPayload,
    pub run_id: Option<String>,
    pub reconstructed_payload: bool,
}

pub fn resolve_summary_source(
    job: &MeetingJob,
    members: &[MeetingMember],
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
        return summary_source_from_run(job, members, run);
    }

    if let Some(active_run_id) = job.active_summary_run_id.as_deref() {
        let run = job
            .summary_runs
            .iter()
            .find(|run| run.id == active_run_id)
            .ok_or_else(|| "导出失败：当前活动的 AI 总结版本不存在。".to_string())?;
        return summary_source_from_run(job, members, run);
    }

    if has_legacy_summary(job) {
        let mut result = AiSummaryResult {
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
        };
        let payload = reconstruct_verified_payload(job, &result, members, "", None)?;
        make_payload_authoritative(&mut result, &payload, true);
        return Ok(SummaryExportSource {
            result,
            minutes_payload: payload,
            run_id: None,
            reconstructed_payload: false,
        });
    }

    Err("导出失败：当前任务没有可导出的 AI 总结。".into())
}

fn summary_source_from_run(
    job: &MeetingJob,
    members: &[MeetingMember],
    run: &AiSummaryRun,
) -> LocalResult<SummaryExportSource> {
    if run.status != "completed" {
        return Err(format!("导出失败：AI 总结版本 {} 尚未完成。", run.id));
    }

    let mut result = run
        .result
        .clone()
        .ok_or_else(|| format!("导出失败：AI 总结版本 {} 没有结果数据。", run.id))?;
    let (mut payload, reconstructed_payload) = match run.minutes_payload.clone() {
        Some(payload) => {
            validate_payload_source(&payload, &run.id)?;
            validate_speaker_coverage(job, &payload)?;
            (payload, false)
        }
        None => (
            reconstruct_verified_payload(
                job,
                &result,
                members,
                &run.template_id,
                Some(run.id.clone()),
            )?,
            true,
        ),
    };
    mark_missing_ai_speakers(job, &mut payload);
    make_payload_authoritative(&mut result, &payload, reconstructed_payload);

    Ok(SummaryExportSource {
        result,
        minutes_payload: payload,
        run_id: Some(run.id.clone()),
        reconstructed_payload,
    })
}

pub(crate) fn reconstruct_verified_payload(
    job: &MeetingJob,
    result: &AiSummaryResult,
    members: &[MeetingMember],
    template_id: &str,
    source_summary_run_id: Option<String>,
) -> LocalResult<MeetingMinutesPayload> {
    let mut payload = super::derive_meeting_minutes_payload(
        job,
        result,
        members,
        template_id,
        source_summary_run_id,
    );
    super::add_missing_transcript_speakers(job, &mut payload, members);
    mark_missing_ai_speakers(job, &mut payload);
    validate_speaker_coverage(job, &payload)?;
    Ok(payload)
}

fn validate_payload_source(payload: &MeetingMinutesPayload, run_id: &str) -> LocalResult<()> {
    if let Some(payload_run_id) = payload
        .source_summary_run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if payload_run_id != run_id {
            return Err(format!(
                "导出失败：AI 总结版本 {run_id} 的结构化会议纪要来自其他版本 {payload_run_id}。"
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_speaker_coverage(
    job: &MeetingJob,
    payload: &MeetingMinutesPayload,
) -> LocalResult<()> {
    if !job.diarization_status.is_verified() {
        return Ok(());
    }
    let segments = &job.speaker_segments;
    let transcript_speakers = segments
        .iter()
        .filter_map(|segment| segment.speaker.as_deref())
        .map(normalize_speaker)
        .filter(|speaker| !speaker.is_empty())
        .collect::<HashSet<_>>();
    let participant_speakers = payload
        .participants
        .iter()
        .flat_map(|participant| [&participant.speaker_label, &participant.resolved_name])
        .map(|speaker| normalize_speaker(speaker))
        .filter(|speaker| !speaker.is_empty())
        .collect::<HashSet<_>>();
    let report_speakers = payload
        .speaker_reports
        .iter()
        .flat_map(|report| [&report.speaker_label, &report.resolved_name])
        .map(|speaker| normalize_speaker(speaker))
        .filter(|speaker| !speaker.is_empty())
        .collect::<HashSet<_>>();
    let mut missing = transcript_speakers
        .into_iter()
        .filter(|speaker| {
            !participant_speakers.contains(speaker) || !report_speakers.contains(speaker)
        })
        .collect::<Vec<_>>();
    missing.sort();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "导出失败：结构化会议纪要缺少逐字稿讲话人：{}。",
            missing.join("、")
        ))
    }
}

fn make_payload_authoritative(
    result: &mut AiSummaryResult,
    payload: &MeetingMinutesPayload,
    allow_unstructured_fallback: bool,
) {
    let original_overview = result.overview.trim().to_string();
    let has_speaker_content = payload.speaker_reports.iter().any(|report| {
        !report.weekly_summary.is_empty()
            || !report.next_week_plan.is_empty()
            || !report.summary.is_empty()
    });
    let mut lines = Vec::new();
    append_section(&mut lines, "决策", &result.decisions);
    let actions = result
        .action_items
        .iter()
        .filter(|item| !item.task.trim().is_empty())
        .map(|item| {
            format!(
                "任务：{}；负责人：{}；截止日期：{}",
                item.task.trim(),
                value_or_pending(&item.owner),
                value_or_pending(&item.due_date)
            )
        })
        .collect::<Vec<_>>();
    append_section(&mut lines, "行动项", &actions);
    append_section(&mut lines, "风险", &result.risks);
    append_section(&mut lines, "跟进事项", &result.follow_ups);
    if allow_unstructured_fallback
        && !has_speaker_content
        && payload.global_summary.is_empty()
        && !original_overview.is_empty()
    {
        lines.insert(0, original_overview);
    }
    result.overview = lines.join("\n");
}

pub(crate) fn mark_missing_ai_speakers(job: &MeetingJob, payload: &mut MeetingMinutesPayload) {
    if !job.diarization_status.is_verified() {
        return;
    }
    let segments = &job.speaker_segments;
    let transcript_speakers = segments
        .iter()
        .filter_map(|segment| segment.speaker.as_deref())
        .map(normalize_speaker)
        .filter(|speaker| !speaker.is_empty())
        .collect::<HashSet<_>>();
    for report in &mut payload.speaker_reports {
        let speaker = normalize_speaker(if report.speaker_label.trim().is_empty() {
            &report.resolved_name
        } else {
            &report.speaker_label
        });
        if transcript_speakers.contains(&speaker)
            && report.weekly_summary.is_empty()
            && report.next_week_plan.is_empty()
            && report.summary.is_empty()
        {
            report.match_status = "missing_from_ai".into();
            if let Some(participant) = payload.participants.iter_mut().find(|participant| {
                normalize_speaker(&participant.speaker_label) == speaker
                    || normalize_speaker(&participant.resolved_name) == speaker
            }) {
                participant.match_status = "missing_from_ai".into();
            }
        }
    }
}

fn append_section(lines: &mut Vec<String>, heading: &str, items: &[String]) {
    if items.iter().all(|item| item.trim().is_empty()) {
        return;
    }
    lines.push(format!("{heading}："));
    lines.extend(
        items
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(str::to_string),
    );
}

fn value_or_pending(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "待定"
    } else {
        trimmed
    }
}

fn normalize_speaker(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('【')
        .trim_end_matches('】')
        .trim_end_matches("（未发言）")
        .trim_end_matches("(未发言)")
        .trim()
        .to_string()
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
            minutes_payload: Some(MeetingMinutesPayload::default()),
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

        let source = resolve_summary_source(&job, &[], Some("run-requested")).unwrap();

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

        let error = resolve_summary_source(&job, &[], Some("run-missing")).unwrap_err();

        assert!(error.contains("run-missing"));
    }

    #[test]
    fn active_run_is_used_when_no_version_is_requested() {
        let job = MeetingJob {
            active_summary_run_id: Some("run-active".into()),
            summary_runs: vec![completed_run("run-active", "活动版本")],
            ..MeetingJob::default()
        };

        let source = resolve_summary_source(&job, &[], None).unwrap();

        assert_eq!(source.run_id.as_deref(), Some("run-active"));
    }

    #[test]
    fn export_rejects_payload_missing_a_transcript_speaker() {
        let mut run = completed_run("run-active", "活动版本");
        run.minutes_payload = Some(MeetingMinutesPayload::default());
        let job = MeetingJob {
            enable_speaker: true,
            diarization_status: crate::domain::asr::DiarizationStatus::Completed,
            speaker_segments: vec![crate::local_db::TranscriptSegment {
                speaker: Some("李兰".into()),
                ..crate::local_db::TranscriptSegment::default()
            }],
            active_summary_run_id: Some("run-active".into()),
            summary_runs: vec![run],
            ..MeetingJob::default()
        };

        let error = resolve_summary_source(&job, &[], None).unwrap_err();

        assert!(error.contains("李兰"));
    }

    #[test]
    fn degraded_diarization_does_not_require_speaker_payload() {
        let mut run = completed_run("run-active", "活动版本");
        run.minutes_payload = Some(MeetingMinutesPayload::default());
        let job = MeetingJob {
            enable_speaker: true,
            diarization_status: crate::domain::asr::DiarizationStatus::Unavailable,
            speaker_segments: vec![crate::local_db::TranscriptSegment {
                speaker: Some("历史残留标签".into()),
                ..crate::local_db::TranscriptSegment::default()
            }],
            active_summary_run_id: Some("run-active".into()),
            summary_runs: vec![run],
            ..MeetingJob::default()
        };

        assert!(resolve_summary_source(&job, &[], None).is_ok());
    }

    #[test]
    fn completed_run_without_payload_reconstructs_and_marks_missing_ai_speakers() {
        let mut run = completed_run("run-active", "活动版本");
        run.minutes_payload = None;
        run.result = Some(AiSummaryResult {
            title: "周会".into(),
            overview: "发言内容\n【部门】：人员A\n上周总结：\n1、A事项".into(),
            ..AiSummaryResult::default()
        });
        let job = MeetingJob {
            enable_speaker: true,
            diarization_status: crate::domain::asr::DiarizationStatus::Completed,
            speaker_segments: vec![
                crate::local_db::TranscriptSegment {
                    speaker: Some("人员A".into()),
                    ..crate::local_db::TranscriptSegment::default()
                },
                crate::local_db::TranscriptSegment {
                    speaker: Some("人员B".into()),
                    ..crate::local_db::TranscriptSegment::default()
                },
            ],
            active_summary_run_id: Some("run-active".into()),
            summary_runs: vec![run],
            ..MeetingJob::default()
        };

        let source = resolve_summary_source(&job, &[], None).unwrap();
        let speakers = source
            .minutes_payload
            .speaker_reports
            .iter()
            .map(|report| report.resolved_name.as_str())
            .collect::<Vec<_>>();

        assert!(source.reconstructed_payload);
        assert!(speakers.contains(&"人员A"));
        assert!(speakers.contains(&"人员B"));
        assert_eq!(
            source
                .minutes_payload
                .speaker_reports
                .iter()
                .find(|report| report.resolved_name == "人员B")
                .map(|report| report.match_status.as_str()),
            Some("missing_from_ai")
        );
    }

    #[test]
    fn reconstructed_payload_keeps_unstructured_overview_as_fallback_content() {
        let mut run = completed_run("run-active", "活动版本");
        run.minutes_payload = None;
        run.result = Some(AiSummaryResult {
            title: "周会".into(),
            overview: "旧记录中唯一的会议摘要正文。".into(),
            ..AiSummaryResult::default()
        });
        let job = MeetingJob {
            enable_speaker: true,
            diarization_status: crate::domain::asr::DiarizationStatus::Completed,
            speaker_segments: vec![crate::local_db::TranscriptSegment {
                speaker: Some("人员A".into()),
                ..crate::local_db::TranscriptSegment::default()
            }],
            active_summary_run_id: Some("run-active".into()),
            summary_runs: vec![run],
            ..MeetingJob::default()
        };

        let source = resolve_summary_source(&job, &[], None).unwrap();

        assert_eq!(source.result.overview, "旧记录中唯一的会议摘要正文。");
        assert_eq!(source.minutes_payload.speaker_reports.len(), 1);
        assert_eq!(
            source.minutes_payload.speaker_reports[0].resolved_name,
            "人员A"
        );
    }
}
