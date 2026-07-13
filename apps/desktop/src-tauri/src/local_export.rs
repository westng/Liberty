mod model;
mod parser;
mod renderer;
mod source;
mod xml;

use crate::local_db::{
    self, AiSummaryResult, LocalResult, MeetingJob, MeetingMember, MeetingMinutesInfo,
    MeetingMinutesParticipant, MeetingMinutesPayload, MeetingMinutesSpeakerReport,
    TranscriptSegment,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tauri::AppHandle;

use model::{ExportDocData, SpeechBlock};
use parser::{
    is_missing_value, non_empty, normalize_member_name, parse_overview_to_export_data,
    parse_speaker_header, trim_numbered_prefix,
};
use renderer::export_summary_docx;
use source::{resolve_summary_source, SummaryExportSource};

const FIXED_MEETING_TIME: &str = "9:00";
const FIXED_MEETING_LOCATION: &str = "小会议室";
const FIXED_MEETING_HOST: &str = "冯吉琼";

#[tauri::command]
pub fn export_job_summary_docx(
    app: AppHandle,
    job_id: String,
    summary_run_id: Option<String>,
    file_path: String,
) -> LocalResult<()> {
    if file_path.trim().is_empty() {
        return Err("导出路径不能为空。".into());
    }

    let job = local_db::get_job(&app, &job_id)?;
    let summary_source = resolve_summary_source(&job, summary_run_id.as_deref())?;
    let members = local_db::list_meeting_members(&app)?;
    let export_data = build_export_doc_data_from_source(&job, &summary_source, &members);
    export_summary_docx(&export_data, Path::new(file_path.trim()))
}

fn build_export_doc_data_from_source(
    job: &MeetingJob,
    source: &SummaryExportSource,
    members: &[MeetingMember],
) -> ExportDocData {
    let projection = derive_meeting_minutes_projection(
        job,
        &source.result,
        members,
        &source.template_id,
        source.run_id.clone(),
    );
    let mut payload = if projection.has_summary_speakers {
        projection.payload
    } else {
        source
            .cached_projection
            .as_ref()
            .filter(|cached| cached_projection_matches_source(cached, source.run_id.as_deref()))
            .filter(|cached| !cached.speaker_reports.is_empty())
            .cloned()
            .unwrap_or(projection.payload)
    };

    if let Some(cached) = source
        .cached_projection
        .as_ref()
        .filter(|cached| cached_projection_matches_source(cached, source.run_id.as_deref()))
    {
        apply_cached_identity_metadata(&mut payload, cached);
    }

    build_export_doc_data_from_minutes_payload(job, &source.result, &payload, members)
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
        .payload
}

struct DerivedMinutesProjection {
    payload: MeetingMinutesPayload,
    has_summary_speakers: bool,
}

fn derive_meeting_minutes_projection(
    job: &MeetingJob,
    summary: &AiSummaryResult,
    members: &[MeetingMember],
    template_id: &str,
    source_summary_run_id: Option<String>,
) -> DerivedMinutesProjection {
    let mut data = parse_overview_to_export_data(&summary.overview);
    let has_summary_speakers = !data.speech_blocks.is_empty();
    let mut summary_blocks = std::mem::take(&mut data.speech_blocks);
    let speech_blocks = resolve_speech_blocks(job, &mut summary_blocks, members);

    let resolved_title = non_empty(&data.meeting_name)
        .or_else(|| non_empty(&summary.title))
        .unwrap_or(&job.title);

    let recorder = if is_missing_value(&data.recorder) {
        members
            .iter()
            .find(|member| member.is_recorder)
            .map(|member| member.name.trim().to_string())
            .unwrap_or_default()
    } else {
        data.recorder.trim().to_string()
    };

    let attendees = if is_missing_value(&data.attendees) {
        speech_blocks
            .iter()
            .map(|block| block.name.trim())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join("、")
    } else {
        data.attendees.trim().to_string()
    };

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

    DerivedMinutesProjection {
        has_summary_speakers,
        payload: MeetingMinutesPayload {
            schema_version: 1,
            template_id: template_id.trim().to_string(),
            source_summary_run_id,
            meeting_info: MeetingMinutesInfo {
                meeting_name: resolved_title.to_string(),
                meeting_time: FIXED_MEETING_TIME.to_string(),
                meeting_location: FIXED_MEETING_LOCATION.to_string(),
                recorder,
                attendees,
                absentees: data.absentees.trim().to_string(),
                host: FIXED_MEETING_HOST.to_string(),
                reviewer: data.reviewer.trim().to_string(),
            },
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
        },
    }
}

fn build_export_doc_data_from_minutes_payload(
    job: &MeetingJob,
    summary: &AiSummaryResult,
    payload: &MeetingMinutesPayload,
    members: &[MeetingMember],
) -> ExportDocData {
    let mut data = ExportDocData::default();
    let meeting_info = &payload.meeting_info;
    let resolved_title = non_empty(&meeting_info.meeting_name)
        .or_else(|| non_empty(&summary.title))
        .unwrap_or(&job.title);

    data.title = if resolved_title.ends_with("会议纪要") {
        resolved_title.to_string()
    } else {
        format!("{resolved_title}会议纪要")
    };
    data.meeting_name = resolved_title.to_string();
    data.meeting_time = FIXED_MEETING_TIME.to_string();
    data.meeting_location = FIXED_MEETING_LOCATION.to_string();
    data.recorder = if is_missing_value(&meeting_info.recorder) {
        members
            .iter()
            .find(|member| member.is_recorder)
            .map(|member| member.name.trim().to_string())
            .unwrap_or_default()
    } else {
        meeting_info.recorder.trim().to_string()
    };
    data.attendees = meeting_info.attendees.trim().to_string();
    data.absentees = meeting_info.absentees.trim().to_string();
    data.topics = if !payload.topics.is_empty() {
        payload.topics.join("；")
    } else {
        summary.topics.join("；")
    };
    data.host = FIXED_MEETING_HOST.to_string();
    data.reviewer = meeting_info.reviewer.trim().to_string();
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

fn cached_projection_matches_source(
    cached: &MeetingMinutesPayload,
    source_run_id: Option<&str>,
) -> bool {
    match (
        cached.source_summary_run_id.as_deref(),
        source_run_id
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (Some(cached_run_id), Some(source_run_id)) => cached_run_id == source_run_id,
        (None, Some(_)) => true,
        (None, None) => true,
        (Some(_), None) => false,
    }
}

#[derive(Clone)]
struct CachedSpeakerIdentity {
    keys: Vec<String>,
    member_id: String,
    department: String,
    sort_order: i64,
    match_status: String,
}

fn apply_cached_identity_metadata(
    payload: &mut MeetingMinutesPayload,
    cached: &MeetingMinutesPayload,
) {
    let identities = cached_speaker_identities(cached);

    for participant in &mut payload.participants {
        if let Some(identity) = find_cached_identity(
            &identities,
            [&participant.speaker_label, &participant.resolved_name],
        ) {
            apply_identity_to_participant(participant, identity);
        }
    }

    for report in &mut payload.speaker_reports {
        if let Some(identity) =
            find_cached_identity(&identities, [&report.speaker_label, &report.resolved_name])
        {
            apply_identity_to_report(report, identity);
        }
    }
}

fn cached_speaker_identities(cached: &MeetingMinutesPayload) -> Vec<CachedSpeakerIdentity> {
    let mut identities = Vec::new();

    for participant in &cached.participants {
        push_cached_identity(
            &mut identities,
            &participant.speaker_label,
            &participant.resolved_name,
            &participant.member_id,
            &participant.department,
            participant.sort_order,
            &participant.match_status,
        );
    }

    for report in &cached.speaker_reports {
        push_cached_identity(
            &mut identities,
            &report.speaker_label,
            &report.resolved_name,
            &report.member_id,
            &report.department,
            report.sort_order,
            &report.match_status,
        );
    }

    identities
}

#[allow(clippy::too_many_arguments)]
fn push_cached_identity(
    identities: &mut Vec<CachedSpeakerIdentity>,
    speaker_label: &str,
    resolved_name: &str,
    member_id: &str,
    department: &str,
    sort_order: i64,
    match_status: &str,
) {
    let keys = [speaker_label, resolved_name]
        .into_iter()
        .map(normalize_member_name)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return;
    }

    if identities
        .iter()
        .any(|identity| identity.keys.iter().any(|key| keys.contains(key)))
    {
        return;
    }

    identities.push(CachedSpeakerIdentity {
        keys,
        member_id: member_id.trim().to_string(),
        department: department.trim().to_string(),
        sort_order,
        match_status: match_status.trim().to_string(),
    });
}

fn find_cached_identity<'a, const N: usize>(
    identities: &'a [CachedSpeakerIdentity],
    values: [&str; N],
) -> Option<&'a CachedSpeakerIdentity> {
    let keys = values
        .into_iter()
        .map(normalize_member_name)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    identities
        .iter()
        .find(|identity| identity.keys.iter().any(|key| keys.contains(key)))
}

fn apply_identity_to_participant(
    participant: &mut MeetingMinutesParticipant,
    identity: &CachedSpeakerIdentity,
) {
    if participant.member_id.trim().is_empty() {
        participant.member_id = identity.member_id.clone();
    }
    if participant.department.trim().is_empty() {
        participant.department = identity.department.clone();
    }
    if participant.sort_order == 0 {
        participant.sort_order = identity.sort_order;
    }
    if participant.match_status.trim().is_empty() || participant.match_status == "unmatched" {
        participant.match_status = identity.match_status.clone();
    }
}

fn apply_identity_to_report(
    report: &mut MeetingMinutesSpeakerReport,
    identity: &CachedSpeakerIdentity,
) {
    if report.member_id.trim().is_empty() {
        report.member_id = identity.member_id.clone();
    }
    if report.department.trim().is_empty() {
        report.department = identity.department.clone();
    }
    if report.sort_order == 0 {
        report.sort_order = identity.sort_order;
    }
    if report.match_status.trim().is_empty() || report.match_status == "unmatched" {
        report.match_status = identity.match_status.clone();
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
    let source = if job.speaker_segments.is_empty() {
        &job.transcript_segments
    } else {
        &job.speaker_segments
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
    fn render_document_xml_omits_person_summary_section() {
        let mut archive = zip::ZipArchive::new(Cursor::new(TEMPLATE_DOCX_BYTES)).unwrap();
        let mut document = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        document.read_to_string(&mut xml).unwrap();

        let mut data = sample_export_data();
        data.speech_blocks[0].summary = vec!["不应输出的人员总结".into()];

        let rendered = render_document_xml(&xml, &data).unwrap();
        assert!(rendered.contains("上周总结："));
        assert!(rendered.contains("本周计划："));
        assert!(!rendered.contains("不应输出的人员总结"));
    }

    #[test]
    fn export_summary_docx_writes_content() {
        let path = std::env::temp_dir().join(format!(
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
    fn build_export_doc_data_falls_back_to_overview_when_structure_is_missing() {
        let job = MeetingJob {
            id: "job-1".into(),
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
    fn build_export_doc_data_replaces_placeholder_attendees_and_recorder() {
        let job = MeetingJob {
            id: "job-3".into(),
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
        assert_eq!(data.recorder, "肖明容");
        assert_eq!(data.attendees, "肖明容、李兰");
    }

    #[test]
    fn build_export_doc_data_overrides_fixed_meeting_fields() {
        let overview = SAMPLE_OVERVIEW
            .replace("会议时间：待补充", "会议时间：10:30")
            .replace("会议地点：待补充", "会议地点：大会议室")
            .replace("会议主持人：待补充", "会议主持人：张三");
        let job = MeetingJob {
            id: "job-4".into(),
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
        assert_eq!(data.meeting_time, FIXED_MEETING_TIME);
        assert_eq!(data.meeting_location, FIXED_MEETING_LOCATION);
        assert_eq!(data.host, FIXED_MEETING_HOST);
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
    fn export_prefers_rederived_payload_when_saved_payload_lost_ai_content() {
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
                ..MeetingMinutesSpeakerReport::default()
            }],
            ..MeetingMinutesPayload::default()
        };
        let source = SummaryExportSource {
            result: summary,
            cached_projection: Some(stale_payload),
            template_id: "builtin-formal-meeting-minutes".into(),
            run_id: Some("run-1".into()),
        };

        let data = build_export_doc_data_from_source(&job, &source, &[]);

        assert!(data
            .speech_blocks
            .iter()
            .any(|block| block.name == "肖明容" && !block.weekly_summary.is_empty()));
    }

    #[test]
    fn cached_projection_with_more_items_cannot_replace_summary_people() {
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
        let cached_projection = MeetingMinutesPayload {
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
            cached_projection: Some(cached_projection),
            template_id: "builtin-formal-meeting-minutes".into(),
            run_id: Some("run-authority".into()),
        };

        let data = build_export_doc_data_from_source(&job, &source, &[]);

        assert_eq!(data.speech_blocks.len(), speakers.len());
        assert_eq!(data.speech_blocks[0].name, "段世琼");
        assert_eq!(data.speech_blocks[10].name, "贾世强");
        assert!(data.speech_blocks.iter().all(|block| block
            .weekly_summary
            .iter()
            .all(|item| !item.contains("缓存条目"))));
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

        assert_eq!(data.recorder, "新姓名");
        assert_eq!(data.attendees, "新姓名");
        assert_eq!(data.speech_blocks[0].name, "新姓名");
        assert_eq!(data.speech_blocks[0].department, "新部门");
    }
}
