mod model;
mod output;
mod parser;
mod renderer;
pub(crate) mod source;
mod text;
mod xml;

use crate::{
    application::project_meeting_minutes::{project_meeting_minutes, ProjectMeetingMinutesRequest},
    domain::meeting_minutes::{MeetingMetadata, PersistedMeetingMetadata},
    infrastructure::repositories::ai_summary_runs,
    local_db::{
        self, AiSummaryResult, LocalResult, MeetingJob, MeetingMember, MeetingMinutesInfo,
        MeetingMinutesParticipant, MeetingMinutesPayload, MeetingMinutesSpeakerReport,
        TranscriptSegment,
    },
};
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, Webview};

use model::{ExportDocData, SpeechBlock};
use output::authorized_output_path;
use parser::{
    is_missing_value, non_empty, normalize_member_name, parse_overview_to_export_data,
    parse_speaker_header, trim_numbered_prefix,
};
use renderer::export_summary_docx;
use source::{resolve_summary_source, SummaryExportSource};
use text::TextExportInput;

#[tauri::command]
pub fn export_job_text(
    app: AppHandle,
    webview: Webview,
    input: TextExportInput,
) -> LocalResult<()> {
    text::export_job_text(app, webview, input)
}

#[tauri::command]
pub fn export_job_summary_docx(
    app: AppHandle,
    webview: Webview,
    job_id: String,
    summary_run_id: Option<String>,
    file_path: String,
) -> LocalResult<()> {
    let output_path = authorized_output_path(&webview, &file_path)?;

    let members = local_db::list_meeting_members(&app)?;
    let mut job = local_db::get_job(&app, &job_id)?;
    let mut summary_source = resolve_summary_source(&job, &members, summary_run_id.as_deref())?;
    if summary_source.reconstructed_payload {
        let run_id = summary_source
            .run_id
            .as_deref()
            .ok_or_else(|| "导出失败：重建的会议纪要缺少来源版本。".to_string())?;
        let mut connection = local_db::open_connection(&app)?;
        let backfilled = match ai_summary_runs::backfill_minutes_payload(
            &mut connection,
            &job.id,
            run_id,
            &summary_source.minutes_payload,
        ) {
            Ok(backfilled) => Some(backfilled),
            Err(error) => {
                eprintln!(
                    "[local-export] failed to persist reconstructed payload for run {run_id}: {error}"
                );
                None
            }
        };
        drop(connection);

        if backfilled == Some(false) {
            job = local_db::get_job(&app, &job_id)?;
            summary_source = resolve_summary_source(&job, &members, summary_run_id.as_deref())?;
        }
    }
    let export_data = build_export_doc_data_from_source(&job, &summary_source, &members);
    export_summary_docx(&export_data, &output_path)
}

fn build_export_doc_data_from_source(
    job: &MeetingJob,
    source: &SummaryExportSource,
    members: &[MeetingMember],
) -> ExportDocData {
    build_export_doc_data_from_minutes_payload(
        job,
        &source.result,
        &source.minutes_payload,
        members,
    )
}

#[cfg(test)]
fn build_export_doc_data(
    job: &MeetingJob,
    summary: &AiSummaryResult,
    members: &[MeetingMember],
) -> ExportDocData {
    let payload = derive_meeting_minutes_payload(job, summary, members, "", None);
    build_export_doc_data_from_minutes_payload(job, summary, &payload, members)
}

pub(crate) fn derive_meeting_minutes_payload(
    job: &MeetingJob,
    summary: &AiSummaryResult,
    members: &[MeetingMember],
    template_id: &str,
    source_summary_run_id: Option<String>,
) -> MeetingMinutesPayload {
    derive_meeting_minutes_projection(job, summary, members, template_id, source_summary_run_id)
}

fn derive_meeting_minutes_projection(
    job: &MeetingJob,
    summary: &AiSummaryResult,
    members: &[MeetingMember],
    template_id: &str,
    source_summary_run_id: Option<String>,
) -> MeetingMinutesPayload {
    let mut data = parse_overview_to_export_data(&summary.overview);
    let metadata = project_meeting_minutes(ProjectMeetingMinutesRequest {
        persisted: None,
        ai_metadata: metadata_from_export_data(&data),
    });
    let mut summary_blocks = std::mem::take(&mut data.speech_blocks);
    let speech_blocks = resolve_speech_blocks(job, &mut summary_blocks, members);

    let topics = if !summary.topics.is_empty() {
        summary.topics.clone()
    } else if !is_missing_value(&data.topics) {
        data.topics
            .split(['、', '；', ';'])
            .map(str::trim)
            .filter(|topic| !topic.is_empty())
            .map(str::to_string)
            .collect()
    } else {
        Vec::new()
    };

    MeetingMinutesPayload {
        schema_version: 2,
        meeting_info_source: metadata.source,
        template_id: template_id.trim().to_string(),
        source_summary_run_id,
        meeting_info: minutes_info_from_metadata(metadata.metadata),
        participants: speech_blocks
            .iter()
            .map(|block| block_to_minutes_participant(block, members))
            .collect(),
        speaker_reports: speech_blocks
            .into_iter()
            .map(|block| block_to_minutes_report(block, members))
            .collect(),
        topics,
        global_summary: data.closing_summary,
    }
}

fn build_export_doc_data_from_minutes_payload(
    job: &MeetingJob,
    summary: &AiSummaryResult,
    payload: &MeetingMinutesPayload,
    members: &[MeetingMember],
) -> ExportDocData {
    let mut data = ExportDocData::default();
    let parsed = parse_overview_to_export_data(&summary.overview);
    let metadata = project_meeting_minutes(ProjectMeetingMinutesRequest {
        persisted: Some(PersistedMeetingMetadata {
            schema_version: payload.schema_version,
            source: payload.meeting_info_source,
            metadata: metadata_from_minutes_info(&payload.meeting_info),
        }),
        ai_metadata: metadata_from_export_data(&parsed),
    });
    for warning in &metadata.warnings {
        eprintln!("[meeting-minutes] warning={warning}");
    }
    let meeting_info = minutes_info_from_metadata(metadata.metadata);
    let resolved_title = non_empty(&meeting_info.meeting_name)
        .or_else(|| non_empty(&summary.title))
        .unwrap_or(&job.title);

    data.title = if resolved_title.ends_with("会议纪要") {
        resolved_title.to_string()
    } else {
        format!("{resolved_title}会议纪要")
    };
    data.meeting_name = resolved_title.to_string();
    data.meeting_time = meeting_info.meeting_time;
    data.meeting_location = meeting_info.meeting_location;
    data.recorder = meeting_info.recorder;
    data.attendees = meeting_info.attendees.trim().to_string();
    data.absentees = meeting_info.absentees.trim().to_string();
    data.topics = if !payload.topics.is_empty() {
        payload.topics.join("；")
    } else {
        summary.topics.join("；")
    };
    data.host = meeting_info.host;
    data.reviewer = meeting_info.reviewer;
    data.closing_summary = payload.global_summary.clone();
    data.fallback_overview = summary
        .overview
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    data.speech_blocks = payload
        .speaker_reports
        .iter()
        .map(|report| minutes_report_to_block(report, members))
        .collect();

    sort_speech_blocks(&mut data.speech_blocks, members);

    if !data.speech_blocks.is_empty() {
        data.attendees = data
            .speech_blocks
            .iter()
            .map(|block| block.name.trim())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join("、");
    }

    if data.speech_blocks.iter().all(|block| {
        block.weekly_summary.is_empty()
            && block.next_week_plan.is_empty()
            && block.summary.is_empty()
    }) {
        let fallback_items = build_fallback_overview_items(summary, &data);
        if !fallback_items.is_empty() {
            if let Some(first_block) = data.speech_blocks.first_mut() {
                first_block.summary = fallback_items;
            } else {
                data.speech_blocks.push(SpeechBlock {
                    department: "会议纪要".into(),
                    name: "摘要".into(),
                    summary: fallback_items,
                    original_index: 0,
                    ..SpeechBlock::default()
                });
            }
        }
    }

    data
}

fn metadata_from_export_data(data: &ExportDocData) -> MeetingMetadata {
    MeetingMetadata {
        meeting_name: data.meeting_name.clone(),
        meeting_time: data.meeting_time.clone(),
        meeting_location: data.meeting_location.clone(),
        recorder: data.recorder.clone(),
        attendees: data.attendees.clone(),
        absentees: data.absentees.clone(),
        host: data.host.clone(),
        reviewer: data.reviewer.clone(),
    }
}

fn metadata_from_minutes_info(info: &MeetingMinutesInfo) -> MeetingMetadata {
    MeetingMetadata {
        meeting_name: info.meeting_name.clone(),
        meeting_time: info.meeting_time.clone(),
        meeting_location: info.meeting_location.clone(),
        recorder: info.recorder.clone(),
        attendees: info.attendees.clone(),
        absentees: info.absentees.clone(),
        host: info.host.clone(),
        reviewer: info.reviewer.clone(),
    }
}

fn minutes_info_from_metadata(metadata: MeetingMetadata) -> MeetingMinutesInfo {
    MeetingMinutesInfo {
        meeting_name: metadata.meeting_name,
        meeting_time: metadata.meeting_time,
        meeting_location: metadata.meeting_location,
        recorder: metadata.recorder,
        attendees: metadata.attendees,
        absentees: metadata.absentees,
        host: metadata.host,
        reviewer: metadata.reviewer,
    }
}

fn resolve_speech_blocks(
    job: &MeetingJob,
    summary_blocks: &mut Vec<SpeechBlock>,
    members: &[MeetingMember],
) -> Vec<SpeechBlock> {
    let mut speech_blocks = merge_summary_speech_blocks(summary_blocks);
    if speech_blocks.is_empty() {
        speech_blocks = collect_speaker_names(job)
            .into_iter()
            .enumerate()
            .map(|(index, name)| SpeechBlock {
                name,
                original_index: index,
                ..SpeechBlock::default()
            })
            .collect();
    }

    sort_speech_blocks(&mut speech_blocks, members);
    speech_blocks
}

fn merge_summary_speech_blocks(summary_blocks: &mut Vec<SpeechBlock>) -> Vec<SpeechBlock> {
    let mut merged = Vec::<SpeechBlock>::new();
    let mut block_index_by_name = HashMap::<String, usize>::new();

    for mut block in summary_blocks.drain(..) {
        let normalized_name = normalize_member_name(&block.name);
        if normalized_name.is_empty() {
            continue;
        }

        if let Some(existing_index) = block_index_by_name.get(&normalized_name).copied() {
            let existing = &mut merged[existing_index];
            append_unique_items(&mut existing.weekly_summary, block.weekly_summary);
            append_unique_items(&mut existing.next_week_plan, block.next_week_plan);
            append_unique_items(&mut existing.summary, block.summary);
            if existing.department.trim().is_empty() && !block.department.trim().is_empty() {
                existing.department = block.department.trim().to_string();
            }
            continue;
        }

        block.name = normalized_name.clone();
        block.original_index = merged.len();
        block_index_by_name.insert(normalized_name, merged.len());
        merged.push(block);
    }

    merged
}

fn append_unique_items(target: &mut Vec<String>, items: Vec<String>) {
    for item in items {
        if !target.iter().any(|existing| existing == &item) {
            target.push(item);
        }
    }
}

pub(crate) fn add_missing_transcript_speakers(
    job: &MeetingJob,
    payload: &mut MeetingMinutesPayload,
    members: &[MeetingMember],
) {
    let first_recovered_index = payload
        .participants
        .iter()
        .map(|participant| participant.original_index)
        .chain(
            payload
                .speaker_reports
                .iter()
                .map(|report| report.original_index),
        )
        .max()
        .map_or(0, |index| index.saturating_add(1));

    for (offset, speaker) in collect_speaker_names(job).into_iter().enumerate() {
        let normalized = normalize_member_name(&speaker);
        let has_participant = payload.participants.iter().any(|participant| {
            [
                participant.speaker_label.as_str(),
                participant.resolved_name.as_str(),
            ]
            .into_iter()
            .any(|value| normalize_member_name(value) == normalized)
        });
        let has_report = payload.speaker_reports.iter().any(|report| {
            [report.speaker_label.as_str(), report.resolved_name.as_str()]
                .into_iter()
                .any(|value| normalize_member_name(value) == normalized)
        });
        if has_participant && has_report {
            continue;
        }

        let block = SpeechBlock {
            name: speaker,
            original_index: first_recovered_index.saturating_add(offset),
            ..SpeechBlock::default()
        };
        if !has_participant {
            let mut participant = block_to_minutes_participant(&block, members);
            participant.match_status = "missing_from_ai".into();
            payload.participants.push(participant);
        }
        if !has_report {
            let mut report = block_to_minutes_report(block, members);
            report.match_status = "missing_from_ai".into();
            payload.speaker_reports.push(report);
        }
    }
}

fn block_to_minutes_participant(
    block: &SpeechBlock,
    members: &[MeetingMember],
) -> MeetingMinutesParticipant {
    let member = find_member_for_name(members, &block.name);
    MeetingMinutesParticipant {
        speaker_label: block.name.trim().to_string(),
        member_id: member.map(|member| member.id.clone()).unwrap_or_default(),
        resolved_name: member
            .map(|member| member.name.trim().to_string())
            .unwrap_or_else(|| block.name.trim().to_string()),
        department: member
            .and_then(|member| non_empty(&member.department).map(str::to_string))
            .or_else(|| non_empty(&block.department).map(str::to_string))
            .unwrap_or_default(),
        sort_order: member.map(|member| member.sort_order).unwrap_or(0),
        original_index: block.original_index,
        match_status: if member.is_some() {
            "matched".into()
        } else {
            "unmatched".into()
        },
    }
}

fn block_to_minutes_report(
    block: SpeechBlock,
    members: &[MeetingMember],
) -> MeetingMinutesSpeakerReport {
    let member = find_member_for_name(members, &block.name);
    MeetingMinutesSpeakerReport {
        speaker_label: block.name.trim().to_string(),
        member_id: member.map(|member| member.id.clone()).unwrap_or_default(),
        resolved_name: member
            .map(|member| member.name.trim().to_string())
            .unwrap_or_else(|| block.name.trim().to_string()),
        department: member
            .and_then(|member| non_empty(&member.department).map(str::to_string))
            .or_else(|| non_empty(&block.department).map(str::to_string))
            .unwrap_or_default(),
        sort_order: member.map(|member| member.sort_order).unwrap_or(0),
        original_index: block.original_index,
        match_status: if member.is_some() {
            "matched".into()
        } else {
            "unmatched".into()
        },
        weekly_summary: block.weekly_summary,
        next_week_plan: block.next_week_plan,
        summary: block.summary,
    }
}

fn minutes_report_to_block(
    report: &MeetingMinutesSpeakerReport,
    members: &[MeetingMember],
) -> SpeechBlock {
    let member = find_member_for_report(members, report);
    SpeechBlock {
        department: member
            .and_then(|member| non_empty(&member.department).map(str::to_string))
            .or_else(|| non_empty(&report.department).map(str::to_string))
            .unwrap_or_default(),
        name: member
            .map(|member| member.name.trim().to_string())
            .or_else(|| non_empty(&report.resolved_name).map(str::to_string))
            .or_else(|| non_empty(&report.speaker_label).map(str::to_string))
            .unwrap_or_default(),
        weekly_summary: report.weekly_summary.clone(),
        next_week_plan: report.next_week_plan.clone(),
        summary: report.summary.clone(),
        original_index: report.original_index,
    }
}

fn find_member_for_report<'a>(
    members: &'a [MeetingMember],
    report: &MeetingMinutesSpeakerReport,
) -> Option<&'a MeetingMember> {
    if !report.member_id.trim().is_empty() {
        if let Some(member) = members.iter().find(|member| member.id == report.member_id) {
            return Some(member);
        }
    }

    find_member_for_name(members, &report.resolved_name)
        .or_else(|| find_member_for_name(members, &report.speaker_label))
}

fn find_member_for_name<'a>(members: &'a [MeetingMember], name: &str) -> Option<&'a MeetingMember> {
    let normalized_name = normalize_member_name(name);
    members
        .iter()
        .find(|member| normalize_member_name(&member.name) == normalized_name)
}

fn sort_speech_blocks(blocks: &mut [SpeechBlock], members: &[MeetingMember]) {
    for block in blocks.iter_mut() {
        if let Some(member) = members.iter().find(|member| {
            normalize_member_name(&member.name) == normalize_member_name(&block.name)
        }) {
            if !member.department.trim().is_empty() {
                block.department = member.department.trim().to_string();
            }
            block.name = member.name.trim().to_string();
        }
    }

    blocks.sort_by(|left, right| {
        let left_member = members.iter().find(|member| {
            normalize_member_name(&member.name) == normalize_member_name(&left.name)
        });
        let right_member = members.iter().find(|member| {
            normalize_member_name(&member.name) == normalize_member_name(&right.name)
        });

        match (left_member, right_member) {
            (Some(left_member), Some(right_member)) => left_member
                .sort_order
                .cmp(&right_member.sort_order)
                .then_with(|| left.original_index.cmp(&right.original_index)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.original_index.cmp(&right.original_index),
        }
    });
}

fn collect_speaker_names(job: &MeetingJob) -> Vec<String> {
    let source = if job.diarization_status.is_verified() && !job.speaker_segments.is_empty() {
        &job.speaker_segments
    } else {
        &job.transcript_segments
    };

    collect_speaker_names_from_segments(source)
}

fn collect_speaker_names_from_segments(segments: &[TranscriptSegment]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();

    for segment in segments {
        let name = normalize_member_name(segment.speaker.as_deref().unwrap_or(""));
        if name.is_empty() {
            continue;
        }

        if seen.insert(name.clone()) {
            names.push(name);
        }
    }

    names
}

fn build_fallback_overview_items(summary: &AiSummaryResult, data: &ExportDocData) -> Vec<String> {
    let mut items = Vec::new();

    if !data.closing_summary.is_empty() {
        items.extend(data.closing_summary.iter().cloned());
    }

    if items.is_empty() {
        for line in &data.fallback_overview {
            if is_metadata_line(line) || is_section_heading(line) {
                continue;
            }

            let trimmed = trim_numbered_prefix(line)
                .trim()
                .trim_matches('【')
                .trim_matches('】')
                .trim();
            if trimmed.is_empty() {
                continue;
            }

            items.push(trimmed.to_string());
        }
    }

    if items.is_empty() {
        for topic in &summary.topics {
            let topic = topic.trim();
            if !topic.is_empty() {
                items.push(topic.to_string());
            }
        }
    }

    items.into_iter().fold(Vec::new(), |mut acc, item| {
        if !acc.iter().any(|existing| existing == &item) {
            acc.push(item);
        }
        acc
    })
}

fn is_metadata_line(line: &str) -> bool {
    [
        "会议名称：",
        "会议时间：",
        "会议地点：",
        "记录人：",
        "出席人员：",
        "缺席人员：",
        "主要议题：",
        "会议主持人：",
        "审阅：",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}

fn is_section_heading(line: &str) -> bool {
    matches!(
        line.trim().trim_end_matches(['：', ':']).trim(),
        "发言内容" | "上周总结" | "本周计划" | "总结"
    ) || parse_speaker_header(line).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_db::AiSummaryRun;
    use crate::local_export::renderer::{render_document_xml, TEMPLATE_DOCX_BYTES};
    use std::{
        fs,
        io::{Cursor, Read},
        time::{SystemTime, UNIX_EPOCH},
    };
    use zip::ZipArchive;

    const SAMPLE_OVERVIEW: &str = "会议名称：标准录音 16\n会议时间：待补充\n会议地点：待补充\n记录人：待补充\n\n出席人员：待补充\n缺席人员：待补充\n主要议题：五一假期接待准备、卫生安全检查、团队与市场拓展、员工工作安排\n会议主持人：待补充\n审阅：待补充\n\n发言内容\n\n【营销部】：李兰\n上周总结：\n1、接待福州社团三天的工作已协调。\n2、交易所协助餐厅事宜。\n\n本周计划：\n1、接待五一成都林总的团队。\n2、全力接待五一期间的接待。\n\n【办公室】：肖明容\n上周总结：\n1、上线两间，下线四间，入住率5%。\n\n本周计划：\n1、统计本周数据，协助工作。\n\n总结：\n1、各部门需做好五一节前准备。\n2、温泉部注意安全。";

    fn sample_export_data() -> ExportDocData {
        let mut data = parse_overview_to_export_data(SAMPLE_OVERVIEW);
        data.title = "标准录音 16会议纪要".into();
        data.meeting_name = "标准录音 16".into();
        data.attendees = "李兰、肖明容".into();
        data.recorder = "肖明容".into();
        data
    }

    #[test]
    fn parse_overview_keeps_global_summary_separate() {
        let data = parse_overview_to_export_data(SAMPLE_OVERVIEW);
        assert_eq!(data.speech_blocks.len(), 2);
        assert_eq!(data.speech_blocks[0].name, "李兰");
        assert!(data.speech_blocks[0].summary.is_empty());
        assert_eq!(data.closing_summary.len(), 2);
    }

    #[test]
    fn parse_overview_accepts_ascii_colon_section_labels() {
        let overview = "会议名称: 周会\n会议时间: 待补充\n会议地点: 待补充\n记录人: 待补充\n\n发言内容\n\n【办公室】: 肖明容\n上周总结:\n1、统计相关数据：客房58间入住15%，温泉575人。\n2、协助工作。\n\n本周计划:\n1、继续统计。\n2、协助工作。";
        let data = parse_overview_to_export_data(overview);

        assert_eq!(data.meeting_name, "周会");
        assert_eq!(data.speech_blocks.len(), 1);
        assert_eq!(data.speech_blocks[0].department, "办公室");
        assert_eq!(data.speech_blocks[0].name, "肖明容");
        assert_eq!(data.speech_blocks[0].weekly_summary.len(), 2);
        assert_eq!(
            data.speech_blocks[0].weekly_summary[0],
            "1、统计相关数据：客房58间入住15%，温泉575人。"
        );
        assert_eq!(data.speech_blocks[0].next_week_plan.len(), 2);
    }

    #[test]
    fn render_document_xml_contains_real_content() {
        let mut archive = zip::ZipArchive::new(Cursor::new(TEMPLATE_DOCX_BYTES)).unwrap();
        let mut document = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        document.read_to_string(&mut xml).unwrap();

        let rendered = render_document_xml(&xml, &sample_export_data()).unwrap();
        assert!(rendered.contains("标准录音 16会议纪要"));
        assert!(rendered.contains("记录人： 肖明容"));
        assert!(rendered.contains("李兰"));
        assert!(rendered.contains("接待福州社团三天的工作已协调"));
        assert!(rendered.contains("统计本周数据，协助工作"));
    }

    #[test]
    fn render_document_xml_keeps_person_summary_section() {
        let mut archive = zip::ZipArchive::new(Cursor::new(TEMPLATE_DOCX_BYTES)).unwrap();
        let mut document = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        document.read_to_string(&mut xml).unwrap();

        let mut data = sample_export_data();
        data.speech_blocks[0].summary = vec!["需要输出的人员总结".into()];

        let rendered = render_document_xml(&xml, &data).unwrap();
        assert!(rendered.contains("上周总结："));
        assert!(rendered.contains("本周计划："));
        assert!(rendered.contains("个人总结："));
        assert!(rendered.contains("需要输出的人员总结"));
    }

    #[test]
    fn export_summary_docx_writes_content() {
        let temp_dir = fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = temp_dir.join(format!(
            "liberty-export-test-{}.docx",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        export_summary_docx(&sample_export_data(), &path).unwrap();
        let bytes = fs::read(&path).unwrap();
        let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let xml = {
            let mut file = zip.by_name("word/document.xml").unwrap();
            let mut buffer = String::new();
            file.read_to_string(&mut buffer).unwrap();
            buffer
        };

        assert!(xml.contains("李兰"));
        assert!(xml.contains("肖明容"));
        assert!(xml.contains("接待五一成都林总的团队"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn completed_run_payload_people_survive_overview_projection_in_document_xml() {
        let run_id = "run-payload-authority";
        let people = ["人员A", "人员B"];
        let payload = MeetingMinutesPayload {
            source_summary_run_id: Some(run_id.into()),
            meeting_info: MeetingMinutesInfo {
                meeting_name: "周会".into(),
                ..MeetingMinutesInfo::default()
            },
            participants: people
                .iter()
                .enumerate()
                .map(|(index, name)| MeetingMinutesParticipant {
                    speaker_label: (*name).into(),
                    resolved_name: (*name).into(),
                    original_index: index,
                    ..MeetingMinutesParticipant::default()
                })
                .collect(),
            speaker_reports: people
                .iter()
                .enumerate()
                .map(|(index, name)| MeetingMinutesSpeakerReport {
                    speaker_label: (*name).into(),
                    resolved_name: (*name).into(),
                    original_index: index,
                    weekly_summary: vec![format!("payload-{name}-事项")],
                    ..MeetingMinutesSpeakerReport::default()
                })
                .collect(),
            ..MeetingMinutesPayload::default()
        };
        let run = AiSummaryRun {
            id: run_id.into(),
            job_id: "job-payload-authority".into(),
            status: "completed".into(),
            result: Some(AiSummaryResult {
                title: "周会".into(),
                overview: "发言内容\n【部门】：人员A\n上周总结：\n1、overview-仅有-A".into(),
                ..AiSummaryResult::default()
            }),
            minutes_payload: Some(payload),
            ..AiSummaryRun::default()
        };
        let job = MeetingJob {
            id: "job-payload-authority".into(),
            title: "周会".into(),
            enable_speaker: true,
            transcript_segments: people
                .iter()
                .enumerate()
                .map(|(index, name)| TranscriptSegment {
                    id: format!("segment-{index}"),
                    speaker: Some((*name).into()),
                    text: format!("{name}发言"),
                    ..TranscriptSegment::default()
                })
                .collect(),
            active_summary_run_id: Some(run_id.into()),
            summary_runs: vec![run],
            ..MeetingJob::default()
        };

        let source = resolve_summary_source(&job, &[], Some(run_id)).unwrap();
        let export_data = build_export_doc_data_from_source(&job, &source, &[]);
        let temp_dir = fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = temp_dir.join(format!(
            "liberty-payload-authority-{}.docx",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        export_summary_docx(&export_data, &path).unwrap();
        let bytes = fs::read(&path).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let document_xml = {
            let mut document = archive.by_name("word/document.xml").unwrap();
            let mut xml = String::new();
            document.read_to_string(&mut xml).unwrap();
            xml
        };

        assert!(document_xml.contains("人员A"));
        assert!(document_xml.contains("payload-人员A-事项"));
        assert!(document_xml.contains("人员B"));
        assert!(document_xml.contains("payload-人员B-事项"));
        assert!(!document_xml.contains("overview-仅有-A"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn build_export_doc_data_falls_back_to_overview_when_structure_is_missing() {
        let job = MeetingJob {
            id: "job-1".into(),
            source: "local".into(),
            title: "例会".into(),
            source_files: Vec::new(),
            duration_minutes: 0,
            processing_started_at_ms: None,
            processing_finished_at_ms: None,
            processing_duration_seconds: None,
            progress_percent: None,
            progress_message: None,
            created_at: String::new(),
            hotwords: Vec::new(),
            lang: "zh".into(),
            enable_speaker: true,
            summary_template: "标准会议纪要".into(),
            upload_status: "completed".into(),
            asr_status: "completed".into(),
            summary_status: "completed".into(),
            overall_status: "completed".into(),
            failure_reason: None,
            transcript_segments: vec![TranscriptSegment {
                id: "seg-1".into(),
                start_ms: 0,
                end_ms: 1,
                speaker: Some("李兰".into()),
                text: "测试".into(),
            }],
            speaker_segments: Vec::new(),
            summary: crate::local_db::MeetingSummary {
                overview: "本周重点完成五一接待准备，并安排节前安全检查。".into(),
                topics: vec!["五一接待准备".into()],
                decisions: Vec::new(),
                action_items: Vec::new(),
                risks: Vec::new(),
                follow_ups: Vec::new(),
            },
            summary_runs: Vec::new(),
            active_summary_run_id: None,
            export_formats: Vec::new(),
            last_exported_at: None,
            process_log: None,
            python_path: None,
            runner_script_path: None,
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "例会".into(),
            overview: "本周重点完成五一接待准备，并安排节前安全检查。".into(),
            topics: vec!["五一接待准备".into()],
            decisions: Vec::new(),
            action_items: Vec::new(),
            risks: Vec::new(),
            follow_ups: Vec::new(),
        };

        let data = build_export_doc_data(&job, &summary, &[]);
        assert_eq!(data.speech_blocks.len(), 1);
        assert_eq!(data.speech_blocks[0].name, "李兰");
        assert_eq!(
            data.speech_blocks[0].summary,
            vec!["本周重点完成五一接待准备，并安排节前安全检查。".to_string()]
        );
    }

    #[test]
    fn build_export_doc_data_keeps_ai_blocks_with_content_even_when_speaker_label_differs() {
        let job = MeetingJob {
            id: "job-2".into(),
            source: "local".into(),
            title: "例会".into(),
            source_files: Vec::new(),
            duration_minutes: 0,
            processing_started_at_ms: None,
            processing_finished_at_ms: None,
            processing_duration_seconds: None,
            progress_percent: None,
            progress_message: None,
            created_at: String::new(),
            hotwords: Vec::new(),
            lang: "zh".into(),
            enable_speaker: true,
            summary_template: "表格版会议纪要".into(),
            upload_status: "completed".into(),
            asr_status: "completed".into(),
            summary_status: "completed".into(),
            overall_status: "completed".into(),
            failure_reason: None,
            transcript_segments: vec![TranscriptSegment {
                id: "seg-1".into(),
                start_ms: 0,
                end_ms: 1,
                speaker: Some("李兰".into()),
                text: "测试".into(),
            }],
            speaker_segments: Vec::new(),
            summary: crate::local_db::MeetingSummary {
                overview: SAMPLE_OVERVIEW.into(),
                topics: vec!["五一接待准备".into()],
                decisions: Vec::new(),
                action_items: Vec::new(),
                risks: Vec::new(),
                follow_ups: Vec::new(),
            },
            summary_runs: Vec::new(),
            active_summary_run_id: None,
            export_formats: Vec::new(),
            last_exported_at: None,
            process_log: None,
            python_path: None,
            runner_script_path: None,
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "例会".into(),
            overview: SAMPLE_OVERVIEW.into(),
            topics: vec!["五一接待准备".into()],
            decisions: Vec::new(),
            action_items: Vec::new(),
            risks: Vec::new(),
            follow_ups: Vec::new(),
        };

        let data = build_export_doc_data(&job, &summary, &[]);
        assert_eq!(data.speech_blocks.len(), 2);
        assert_eq!(data.speech_blocks[0].name, "李兰");
        assert_eq!(data.speech_blocks[1].name, "肖明容");
        assert_eq!(
            data.speech_blocks[1].weekly_summary,
            vec!["1、上线两间，下线四间，入住率5%。".to_string()]
        );
    }

    #[test]
    fn build_export_doc_data_keeps_missing_metadata_empty_for_renderer() {
        let job = MeetingJob {
            id: "job-3".into(),
            source: "local".into(),
            title: "例会".into(),
            source_files: Vec::new(),
            duration_minutes: 0,
            processing_started_at_ms: None,
            processing_finished_at_ms: None,
            processing_duration_seconds: None,
            progress_percent: None,
            progress_message: None,
            created_at: String::new(),
            hotwords: Vec::new(),
            lang: "zh".into(),
            enable_speaker: true,
            summary_template: "表格版会议纪要".into(),
            upload_status: "completed".into(),
            asr_status: "completed".into(),
            summary_status: "completed".into(),
            overall_status: "completed".into(),
            failure_reason: None,
            transcript_segments: vec![
                TranscriptSegment {
                    id: "seg-1".into(),
                    start_ms: 0,
                    end_ms: 1,
                    speaker: Some("李兰".into()),
                    text: "测试".into(),
                },
                TranscriptSegment {
                    id: "seg-2".into(),
                    start_ms: 2,
                    end_ms: 3,
                    speaker: Some("段世琼".into()),
                    text: "测试".into(),
                },
            ],
            speaker_segments: Vec::new(),
            summary: crate::local_db::MeetingSummary {
                overview: SAMPLE_OVERVIEW.into(),
                topics: vec!["五一接待准备".into()],
                decisions: Vec::new(),
                action_items: Vec::new(),
                risks: Vec::new(),
                follow_ups: Vec::new(),
            },
            summary_runs: Vec::new(),
            active_summary_run_id: None,
            export_formats: Vec::new(),
            last_exported_at: None,
            process_log: None,
            python_path: None,
            runner_script_path: None,
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "例会".into(),
            overview: SAMPLE_OVERVIEW.into(),
            topics: vec!["五一接待准备".into()],
            decisions: Vec::new(),
            action_items: Vec::new(),
            risks: Vec::new(),
            follow_ups: Vec::new(),
        };
        let members = vec![MeetingMember {
            id: "member-1".into(),
            name: "肖明容".into(),
            department: "办公室".into(),
            sort_order: 1,
            is_recorder: true,
            created_at: String::new(),
            updated_at: String::new(),
        }];

        let data = build_export_doc_data(&job, &summary, &members);
        assert!(data.recorder.is_empty());
        assert_eq!(data.attendees, "肖明容、李兰");
    }

    #[test]
    fn build_export_doc_data_preserves_ai_meeting_fields() {
        let overview = SAMPLE_OVERVIEW
            .replace("会议时间：待补充", "会议时间：10:30")
            .replace("会议地点：待补充", "会议地点：大会议室")
            .replace("会议主持人：待补充", "会议主持人：张三");
        let job = MeetingJob {
            id: "job-4".into(),
            source: "local".into(),
            title: "例会".into(),
            source_files: Vec::new(),
            duration_minutes: 0,
            processing_started_at_ms: None,
            processing_finished_at_ms: None,
            processing_duration_seconds: None,
            progress_percent: None,
            progress_message: None,
            created_at: String::new(),
            hotwords: Vec::new(),
            lang: "zh".into(),
            enable_speaker: true,
            summary_template: "表格版会议纪要".into(),
            upload_status: "completed".into(),
            asr_status: "completed".into(),
            summary_status: "completed".into(),
            overall_status: "completed".into(),
            failure_reason: None,
            transcript_segments: vec![TranscriptSegment {
                id: "seg-1".into(),
                start_ms: 0,
                end_ms: 1,
                speaker: Some("李兰".into()),
                text: "测试".into(),
            }],
            speaker_segments: Vec::new(),
            summary: crate::local_db::MeetingSummary {
                overview: overview.clone(),
                topics: vec!["五一接待准备".into()],
                decisions: Vec::new(),
                action_items: Vec::new(),
                risks: Vec::new(),
                follow_ups: Vec::new(),
            },
            summary_runs: Vec::new(),
            active_summary_run_id: None,
            export_formats: Vec::new(),
            last_exported_at: None,
            process_log: None,
            python_path: None,
            runner_script_path: None,
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "例会".into(),
            overview,
            topics: vec!["五一接待准备".into()],
            decisions: Vec::new(),
            action_items: Vec::new(),
            risks: Vec::new(),
            follow_ups: Vec::new(),
        };

        let data = build_export_doc_data(&job, &summary, &[]);
        assert_eq!(data.meeting_time, "10:30");
        assert_eq!(data.meeting_location, "大会议室");
        assert_eq!(data.host, "张三");
    }

    #[test]
    fn derive_minutes_payload_preserves_summary_speakers_as_content_authority() {
        let job = MeetingJob {
            id: "job-5".into(),
            title: "周会".into(),
            transcript_segments: vec![
                TranscriptSegment {
                    id: "seg-1".into(),
                    start_ms: 0,
                    end_ms: 1,
                    speaker: Some("李兰".into()),
                    text: "测试".into(),
                },
                TranscriptSegment {
                    id: "seg-2".into(),
                    start_ms: 2,
                    end_ms: 3,
                    speaker: Some("段世琼".into()),
                    text: "测试".into(),
                },
            ],
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "周会".into(),
            overview: SAMPLE_OVERVIEW.into(),
            topics: vec!["五一接待准备".into()],
            ..AiSummaryResult::default()
        };
        let members = vec![MeetingMember {
            id: "member-1".into(),
            name: "段世琼".into(),
            department: "财务部".into(),
            sort_order: 1,
            is_recorder: false,
            created_at: String::new(),
            updated_at: String::new(),
        }];

        let payload = derive_meeting_minutes_payload(
            &job,
            &summary,
            &members,
            "builtin-formal-meeting-minutes",
            Some("run-1".into()),
        );

        assert_eq!(payload.template_id, "builtin-formal-meeting-minutes");
        assert_eq!(payload.source_summary_run_id.as_deref(), Some("run-1"));
        assert_eq!(payload.speaker_reports.len(), 2);
        assert_eq!(payload.speaker_reports[0].resolved_name, "李兰");
        assert_eq!(payload.speaker_reports[0].match_status, "unmatched");
        assert_eq!(payload.speaker_reports[1].resolved_name, "肖明容");
        assert_eq!(payload.speaker_reports[1].weekly_summary.len(), 1);
        assert!(payload
            .speaker_reports
            .iter()
            .all(|report| report.resolved_name != "段世琼"));
    }

    #[test]
    fn export_keeps_saved_payload_authoritative_over_overview_projection() {
        let job = MeetingJob {
            id: "job-7".into(),
            title: "周会".into(),
            transcript_segments: vec![TranscriptSegment {
                id: "seg-1".into(),
                start_ms: 0,
                end_ms: 1,
                speaker: Some("李兰".into()),
                text: "测试".into(),
            }],
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "周会".into(),
            overview: SAMPLE_OVERVIEW.into(),
            topics: vec!["五一接待准备".into()],
            ..AiSummaryResult::default()
        };
        let stale_payload = MeetingMinutesPayload {
            schema_version: 1,
            template_id: "builtin-formal-meeting-minutes".into(),
            meeting_info: MeetingMinutesInfo {
                meeting_name: "周会".into(),
                ..MeetingMinutesInfo::default()
            },
            speaker_reports: vec![MeetingMinutesSpeakerReport {
                speaker_label: "李兰".into(),
                resolved_name: "李兰".into(),
                original_index: 0,
                weekly_summary: vec!["payload 中的李兰事项".into()],
                ..MeetingMinutesSpeakerReport::default()
            }],
            ..MeetingMinutesPayload::default()
        };
        let source = SummaryExportSource {
            result: summary,
            minutes_payload: stale_payload,
            run_id: Some("run-1".into()),
            reconstructed_payload: false,
        };

        let data = build_export_doc_data_from_source(&job, &source, &[]);

        assert_eq!(data.speech_blocks.len(), 1);
        assert_eq!(data.speech_blocks[0].name, "李兰");
        assert_eq!(
            data.speech_blocks[0].weekly_summary,
            vec!["payload 中的李兰事项".to_string()]
        );
    }

    #[test]
    fn overview_projection_cannot_expand_or_replace_payload_people() {
        let speakers = [
            ("营销部", "段世琼"),
            ("营销部", "李兰"),
            ("温泉部", "杨小容"),
            ("客房部", "陈丽"),
            ("餐饮部", "游长春"),
            ("安全部", "周永均"),
            ("工程安保部", "明红"),
            ("运营部", "王英海"),
            ("财务部", "何冬妹"),
            ("办公室", "肖明容"),
            ("董事办", "贾世强"),
        ];
        let overview = format!(
            "发言内容\n{}",
            speakers
                .iter()
                .map(|(department, name)| format!(
                    "【{department}】：{name}\n上周总结：\n1、{name}上周事项\n本周计划：\n1、{name}本周计划"
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let job = MeetingJob {
            id: "job-authority".into(),
            title: "周会".into(),
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "周会".into(),
            overview,
            ..AiSummaryResult::default()
        };
        let minutes_payload = MeetingMinutesPayload {
            source_summary_run_id: Some("run-authority".into()),
            speaker_reports: speakers[..8]
                .iter()
                .map(|(_, name)| MeetingMinutesSpeakerReport {
                    speaker_label: (*name).into(),
                    resolved_name: (*name).into(),
                    weekly_summary: (1..=100)
                        .map(|index| format!("{name}缓存条目 {index}"))
                        .collect(),
                    ..MeetingMinutesSpeakerReport::default()
                })
                .collect(),
            ..MeetingMinutesPayload::default()
        };
        let source = SummaryExportSource {
            result: summary,
            minutes_payload,
            run_id: Some("run-authority".into()),
            reconstructed_payload: false,
        };

        let data = build_export_doc_data_from_source(&job, &source, &[]);

        assert_eq!(data.speech_blocks.len(), 8);
        assert_eq!(data.speech_blocks[0].name, "段世琼");
        assert_eq!(data.speech_blocks[7].name, "王英海");
        assert!(data.speech_blocks.iter().all(|block| block
            .weekly_summary
            .iter()
            .any(|item| item.contains("缓存条目"))));
    }

    #[test]
    fn duplicate_summary_sections_are_merged_without_losing_items() {
        let job = MeetingJob {
            id: "job-duplicate".into(),
            title: "周会".into(),
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "周会".into(),
            overview: "发言内容\n【营销部】：李兰\n上周总结：\n1、事项一\n本周计划：\n1、计划一\n【营销部】：李兰\n上周总结：\n1、事项二\n本周计划：\n1、计划二"
                .into(),
            ..AiSummaryResult::default()
        };

        let payload = derive_meeting_minutes_payload(&job, &summary, &[], "", None);

        assert_eq!(payload.speaker_reports.len(), 1);
        assert_eq!(
            payload.speaker_reports[0].weekly_summary,
            vec!["1、事项一".to_string(), "1、事项二".to_string()]
        );
        assert_eq!(
            payload.speaker_reports[0].next_week_plan,
            vec!["1、计划一".to_string(), "1、计划二".to_string()]
        );
    }

    #[test]
    fn export_from_payload_re_resolves_current_member_profile_by_id() {
        let job = MeetingJob {
            id: "job-6".into(),
            title: "周会".into(),
            ..MeetingJob::default()
        };
        let summary = AiSummaryResult {
            title: "周会".into(),
            overview: "本周完成重点事项。".into(),
            topics: vec!["重点事项".into()],
            ..AiSummaryResult::default()
        };
        let payload = MeetingMinutesPayload {
            schema_version: 1,
            template_id: "builtin-formal-meeting-minutes".into(),
            meeting_info: MeetingMinutesInfo {
                meeting_name: "周会".into(),
                recorder: "待补充".into(),
                ..MeetingMinutesInfo::default()
            },
            speaker_reports: vec![MeetingMinutesSpeakerReport {
                speaker_label: "旧姓名".into(),
                member_id: "member-1".into(),
                resolved_name: "旧姓名".into(),
                department: "旧部门".into(),
                weekly_summary: vec!["完成事项".into()],
                original_index: 0,
                ..MeetingMinutesSpeakerReport::default()
            }],
            ..MeetingMinutesPayload::default()
        };
        let members = vec![MeetingMember {
            id: "member-1".into(),
            name: "新姓名".into(),
            department: "新部门".into(),
            sort_order: 1,
            is_recorder: true,
            created_at: String::new(),
            updated_at: String::new(),
        }];

        let data = build_export_doc_data_from_minutes_payload(&job, &summary, &payload, &members);

        assert!(data.recorder.is_empty());
        assert_eq!(data.attendees, "新姓名");
        assert_eq!(data.speech_blocks[0].name, "新姓名");
        assert_eq!(data.speech_blocks[0].department, "新部门");
    }
}
