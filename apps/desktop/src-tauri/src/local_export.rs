mod model;
mod parser;
mod renderer;
mod xml;

use crate::local_db::{
    self, AiSummaryResult, LocalResult, MeetingJob, MeetingMember, TranscriptSegment,
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

const FIXED_MEETING_TIME: &str = "9:00";
const FIXED_MEETING_LOCATION: &str = "小会议室";
const FIXED_MEETING_HOST: &str = "冯吉琼";

#[tauri::command]
pub fn export_job_summary_docx(
    app: AppHandle,
    job_id: String,
    file_path: String,
) -> LocalResult<()> {
    if file_path.trim().is_empty() {
        return Err("导出路径不能为空。".into());
    }

    let job = local_db::get_job(&app, &job_id)?;
    let summary = resolve_summary_result(&job);
    let members = local_db::list_meeting_members(&app)?;
    let export_data = build_export_doc_data(&job, &summary, &members);
    export_summary_docx(&export_data, Path::new(file_path.trim()))
}

fn resolve_summary_result(job: &MeetingJob) -> AiSummaryResult {
    if let Some(active_run_id) = job.active_summary_run_id.as_deref() {
        if let Some(result) = job
            .summary_runs
            .iter()
            .find(|run| run.id == active_run_id)
            .and_then(|run| run.result.clone())
        {
            return result;
        }
    }

    if let Some(result) = job
        .summary_runs
        .iter()
        .filter(|run| run.result.is_some())
        .max_by(|left, right| left.updated_at.cmp(&right.updated_at))
        .and_then(|run| run.result.clone())
    {
        return result;
    }

    AiSummaryResult {
        title: job.title.clone(),
        overview: job.summary.overview.clone(),
        topics: job.summary.topics.clone(),
        decisions: Vec::new(),
        action_items: Vec::new(),
        risks: Vec::new(),
        follow_ups: Vec::new(),
    }
}

fn build_export_doc_data(
    job: &MeetingJob,
    summary: &AiSummaryResult,
    members: &[MeetingMember],
) -> ExportDocData {
    let mut data = parse_overview_to_export_data(&summary.overview);
    let transcript_speakers = collect_speaker_names(job);
    let mut summary_blocks = std::mem::take(&mut data.speech_blocks);
    let mut blocks_by_name: HashMap<String, SpeechBlock> = HashMap::new();

    for mut block in summary_blocks.drain(..) {
        if normalize_member_name(&block.name).is_empty() {
            continue;
        }

        block.name = normalize_member_name(&block.name);
        blocks_by_name.insert(block.name.clone(), block);
    }

    let mut speech_blocks = Vec::new();
    let mut seen_names = HashSet::new();
    for (index, speaker_name) in transcript_speakers.iter().enumerate() {
        let normalized_name = normalize_member_name(speaker_name);
        if normalized_name.is_empty() || !seen_names.insert(normalized_name.to_string()) {
            continue;
        }

        let mut block = blocks_by_name
            .remove(&normalized_name)
            .unwrap_or_else(|| SpeechBlock {
                name: normalized_name.clone(),
                original_index: index,
                ..SpeechBlock::default()
            });
        block.name = normalized_name.clone();
        block.original_index = index;
        speech_blocks.push(block);
    }

    if transcript_speakers.is_empty() {
        for (_, mut block) in blocks_by_name {
            block.name = normalize_member_name(&block.name);
            if block.name.trim().is_empty() {
                continue;
            }
            block.original_index = transcript_speakers.len() + speech_blocks.len();
            speech_blocks.push(block);
        }
    }

    data.speech_blocks = speech_blocks;

    let resolved_title = non_empty(&data.meeting_name)
        .or_else(|| non_empty(&summary.title))
        .unwrap_or(&job.title);

    data.title = if resolved_title.ends_with("会议纪要") {
        resolved_title.to_string()
    } else {
        format!("{resolved_title}会议纪要")
    };

    if data.meeting_name.trim().is_empty() {
        data.meeting_name = resolved_title.to_string();
    }

    if is_missing_value(&data.recorder) {
        if let Some(member) = members.iter().find(|member| member.is_recorder) {
            data.recorder = member.name.trim().to_string();
        }
    }

    if is_missing_value(&data.topics) && !summary.topics.is_empty() {
        data.topics = summary.topics.join("；");
    }

    sort_speech_blocks(&mut data.speech_blocks, members);

    if is_missing_value(&data.attendees) {
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

    data.meeting_time = FIXED_MEETING_TIME.to_string();
    data.meeting_location = FIXED_MEETING_LOCATION.to_string();
    data.host = FIXED_MEETING_HOST.to_string();

    data
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
        line.trim_end_matches('：'),
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
    fn build_export_doc_data_excludes_members_not_in_meeting() {
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
        assert_eq!(data.speech_blocks.len(), 1);
        assert_eq!(data.speech_blocks[0].name, "李兰");
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
        assert_eq!(data.attendees, "李兰、段世琼");
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
}
