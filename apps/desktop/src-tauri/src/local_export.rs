use crate::local_db::{self, AiSummaryResult, LocalResult, MeetingJob, MeetingMember, TranscriptSegment};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Cursor, Read, Write},
    path::Path,
};
use tauri::AppHandle;
use xmltree::{Element, EmitterConfig, XMLNode};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const TEMPLATE_DOCX_BYTES: &[u8] = include_bytes!("../resources/templates/meeting-minutes.docx");
const FIXED_MEETING_TIME: &str = "9:00";
const FIXED_MEETING_LOCATION: &str = "小会议室";
const FIXED_MEETING_HOST: &str = "冯吉琼";

#[derive(Debug, Clone, Default)]
struct ExportDocData {
    title: String,
    meeting_name: String,
    meeting_time: String,
    meeting_location: String,
    recorder: String,
    attendees: String,
    absentees: String,
    topics: String,
    host: String,
    reviewer: String,
    closing_summary: Vec<String>,
    fallback_overview: Vec<String>,
    speech_blocks: Vec<SpeechBlock>,
}

#[derive(Debug, Clone, Default)]
struct SpeechBlock {
    department: String,
    name: String,
    weekly_summary: Vec<String>,
    next_week_plan: Vec<String>,
    summary: Vec<String>,
    original_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeechSection {
    WeeklySummary,
    NextWeekPlan,
    Summary,
}

#[tauri::command]
pub fn export_job_summary_docx(app: AppHandle, job_id: String, file_path: String) -> LocalResult<()> {
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

fn build_export_doc_data(job: &MeetingJob, summary: &AiSummaryResult, members: &[MeetingMember]) -> ExportDocData {
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

    if data
        .speech_blocks
        .iter()
        .all(|block| block.weekly_summary.is_empty() && block.next_week_plan.is_empty() && block.summary.is_empty())
    {
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

fn export_summary_docx(data: &ExportDocData, output_path: &Path) -> LocalResult<()> {
    let mut archive = ZipArchive::new(Cursor::new(TEMPLATE_DOCX_BYTES))
        .map_err(|err| format!("会议纪要模板读取失败: {err}"))?;
    let output = File::create(output_path).map_err(|err| err.to_string())?;
    let mut writer = ZipWriter::new(output);

    for index in 0..archive.len() {
        let mut source = archive.by_index(index).map_err(|err| err.to_string())?;
        let name = source.name().to_string();
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        writer
            .start_file(name.clone(), options)
            .map_err(|err| err.to_string())?;

        if name == "word/document.xml" {
            let mut xml = String::new();
            source.read_to_string(&mut xml).map_err(|err| err.to_string())?;
            let updated = render_document_xml(&xml, data)?;
            writer
                .write_all(updated.as_bytes())
                .map_err(|err| err.to_string())?;
        } else {
            let mut buffer = Vec::new();
            source.read_to_end(&mut buffer).map_err(|err| err.to_string())?;
            writer.write_all(&buffer).map_err(|err| err.to_string())?;
        }
    }

    writer.finish().map_err(|err| err.to_string())?;
    Ok(())
}

fn render_document_xml(xml: &str, data: &ExportDocData) -> LocalResult<String> {
    let mut root = Element::parse(xml.as_bytes()).map_err(|err| format!("会议纪要模板解析失败: {err}"))?;
    let body = find_child_mut(&mut root, "body").ok_or_else(|| "会议纪要模板缺少文档主体。".to_string())?;
    let title_paragraph = find_child_mut(body, "p").ok_or_else(|| "会议纪要模板缺少标题段落。".to_string())?;
    set_first_text(title_paragraph, &data.title);

    let table = find_child_mut(body, "tbl").ok_or_else(|| "会议纪要模板缺少主表格。".to_string())?;
    let mut rows = child_elements_mut(table, "tr");
    if rows.len() < 7 {
        return Err("会议纪要模板结构不完整，缺少发言内容样板行。".into());
    }

    set_cell_value(rows[0], 1, &fallback_text(&data.meeting_name, "待补充"))?;
    set_cell_value(rows[0], 3, &fallback_text(&data.meeting_time, "待补充"))?;
    set_cell_value(rows[0], 5, &fallback_text(&data.meeting_location, "待补充"))?;
    set_cell_value(rows[0], 6, &format!("记录人： {}", fallback_text(&data.recorder, "待补充")))?;
    set_cell_value(rows[1], 1, &fallback_text(&data.attendees, "待补充"))?;
    set_cell_value(rows[2], 1, &data.absentees)?;
    set_cell_value(rows[3], 1, &fallback_text(&data.topics, "待补充"))?;
    set_cell_value(rows[4], 1, &fallback_text(&data.host, "待补充"))?;
    set_cell_value(rows[4], 3, &data.reviewer)?;

    let sample_row = rows[6].clone();
    while child_elements_count(table, "tr") > 6 {
        remove_last_child_element(table, "tr");
    }

    let speech_blocks = if data.speech_blocks.is_empty() {
        vec![SpeechBlock::default()]
    } else {
        data.speech_blocks.clone()
    };

    for block in speech_blocks {
        let mut row = sample_row.clone();
        fill_speech_row(&mut row, &block)?;
        table.children.push(XMLNode::Element(row));
    }

    let mut output = Vec::new();
    root.write_with_config(
        &mut output,
        EmitterConfig::new()
            .perform_indent(false)
            .write_document_declaration(true),
    )
    .map_err(|err| err.to_string())?;

    String::from_utf8(output).map_err(|err| err.to_string())
}

fn fill_speech_row(row: &mut Element, block: &SpeechBlock) -> LocalResult<()> {
    let mut cells = child_elements_mut(row, "tc");
    if cells.len() < 2 {
        return Err("会议纪要模板的发言内容行结构不正确。".into());
    }

    let mut left_paragraphs = child_elements_mut(cells[0], "p");
    if left_paragraphs.len() < 2 {
        return Err("会议纪要模板的发言人信息单元格结构不正确。".into());
    }

    set_first_text(
        left_paragraphs[0],
        if block.department.trim().is_empty() {
            "待补充部门"
        } else {
            block.department.trim()
        },
    );
    set_first_text(
        left_paragraphs[1],
        if block.name.trim().is_empty() {
            "待补充姓名"
        } else {
            block.name.trim()
        },
    );

    let content_cell = &mut cells[1];
    let paragraphs = child_elements(content_cell, "p");
    if paragraphs.len() < 13 {
        return Err("会议纪要模板的发言内容样板段落数量不足。".into());
    }

    let heading_template = paragraphs[0].clone();
    let item_template = paragraphs[1].clone();
    let summary_template = paragraphs[12].clone();

    content_cell.children.retain(|node| !matches!(node, XMLNode::Element(element) if local_name(&element.name) == "p"));

    append_paragraph(content_cell, clone_with_text(&heading_template, "上周总结："));
    append_section_items(content_cell, &block.weekly_summary, &item_template);
    append_paragraph(content_cell, clone_with_text(&heading_template, "本周计划："));
    append_section_items(content_cell, &block.next_week_plan, &item_template);
    append_paragraph(content_cell, clone_with_text(&heading_template, "总结："));
    let summary_items = if block.summary.is_empty() {
        synthesize_summary(block)
    } else {
        block.summary.clone()
    };
    append_section_items(content_cell, &summary_items, &summary_template);

    Ok(())
}

fn append_section_items(cell: &mut Element, items: &[String], template: &Element) {
    let values = if items.is_empty() {
        vec!["待补充".to_string()]
    } else {
        items.to_vec()
    };

    for (index, item) in values.iter().enumerate() {
        let text = if item.trim().is_empty() {
            format!("{}、待补充", index + 1)
        } else if starts_with_numbered_item(item) {
            item.trim().to_string()
        } else {
            format!("{}、{}", index + 1, item.trim())
        };
        append_paragraph(cell, clone_with_text(template, &text));
    }
}

fn append_paragraph(cell: &mut Element, paragraph: Element) {
    cell.children.push(XMLNode::Element(paragraph));
}

fn parse_overview_to_export_data(overview: &str) -> ExportDocData {
    let mut data = ExportDocData::default();
    let mut current_block: Option<SpeechBlock> = None;
    let mut current_section: Option<SpeechSection> = None;
    let mut in_speech = false;
    let mut in_closing_summary = false;
    let lines = overview
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    data.fallback_overview = lines.iter().map(|line| (*line).to_string()).collect();

    for (index, line) in lines.iter().copied().enumerate() {
        let remaining_lines = &lines[index + 1..];

        if line == "发言内容" {
            in_speech = true;
            in_closing_summary = false;
            if let Some(block) = current_block.take() {
                data.speech_blocks.push(block);
            }
            current_section = None;
            continue;
        }

        if !in_speech {
            if let Some(value) = line.strip_prefix("会议名称：") {
                data.meeting_name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("会议时间：") {
                data.meeting_time = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("会议地点：") {
                data.meeting_location = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("记录人：") {
                data.recorder = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("出席人员：") {
                data.attendees = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("缺席人员：") {
                data.absentees = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("主要议题：") {
                data.topics = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("会议主持人：") {
                data.host = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("审阅：") {
                data.reviewer = value.trim().to_string();
            }
            continue;
        }

        if let Some((department, name)) = parse_speaker_header(line) {
            if let Some(block) = current_block.take() {
                data.speech_blocks.push(block);
            }
            current_block = Some(SpeechBlock {
                department,
                name: normalize_member_name(&name),
                original_index: data.speech_blocks.len(),
                ..SpeechBlock::default()
            });
            in_closing_summary = false;
            current_section = None;
            continue;
        }

        match line.trim_end_matches('：') {
            "上周总结" => {
                current_section = Some(SpeechSection::WeeklySummary);
                continue;
            }
            "本周计划" => {
                current_section = Some(SpeechSection::NextWeekPlan);
                continue;
            }
            "总结" => {
                if current_block.is_some() {
                    let has_future_speaker = remaining_lines
                        .iter()
                        .any(|candidate| parse_speaker_header(candidate).is_some());
                    let should_use_closing_summary =
                        !has_future_speaker && !data.speech_blocks.is_empty();

                    if should_use_closing_summary {
                        if let Some(block) = current_block.take() {
                            data.speech_blocks.push(block);
                        }
                        current_section = None;
                        in_closing_summary = true;
                    } else {
                        current_section = Some(SpeechSection::Summary);
                        in_closing_summary = false;
                    }
                } else {
                    current_section = None;
                    in_closing_summary = true;
                }
                continue;
            }
            _ => {}
        }

        if in_closing_summary {
            data.closing_summary.push(line.to_string());
            continue;
        }

        if let (Some(block), Some(section)) = (&mut current_block, current_section) {
            let target = match section {
                SpeechSection::WeeklySummary => &mut block.weekly_summary,
                SpeechSection::NextWeekPlan => &mut block.next_week_plan,
                SpeechSection::Summary => &mut block.summary,
            };
            target.push(line.to_string());
        }
    }

    if let Some(block) = current_block.take() {
        data.speech_blocks.push(block);
    }

    data
}

fn parse_speaker_header(line: &str) -> Option<(String, String)> {
    let normalized = line.trim().replace('：', ":");
    let (left, right) = normalized.split_once(':')?;
    let department = left.trim().trim_start_matches('【').trim_end_matches('】').trim();
    let name = right.trim().trim_start_matches('【').trim_end_matches('】').trim();

    if department.is_empty() || name.is_empty() {
        return None;
    }

    Some((department.to_string(), name.to_string()))
}

fn sort_speech_blocks(blocks: &mut [SpeechBlock], members: &[MeetingMember]) {
    for block in blocks.iter_mut() {
        if let Some(member) = members
            .iter()
            .find(|member| normalize_member_name(&member.name) == normalize_member_name(&block.name))
        {
            if !member.department.trim().is_empty() {
                block.department = member.department.trim().to_string();
            }
            block.name = member.name.trim().to_string();
        }
    }

    blocks.sort_by(|left, right| {
        let left_member = members
            .iter()
            .find(|member| normalize_member_name(&member.name) == normalize_member_name(&left.name));
        let right_member = members
            .iter()
            .find(|member| normalize_member_name(&member.name) == normalize_member_name(&right.name));

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

fn set_cell_value(row: &mut Element, index: usize, value: &str) -> LocalResult<()> {
    let mut cells = child_elements_mut(row, "tc");
    let cell = cells
        .get_mut(index)
        .ok_or_else(|| format!("会议纪要模板缺少第 {index} 个单元格。"))?;
    set_first_text(cell, value);
    Ok(())
}

fn set_first_text(element: &mut Element, value: &str) {
    if replace_first_text(element, value) {
        return;
    }

    let mut text = Element::new("w:t");
    text.children.push(XMLNode::Text(value.to_string()));
    let mut run = Element::new("w:r");
    run.children.push(XMLNode::Element(text));
    let mut paragraph = Element::new("w:p");
    paragraph.children.push(XMLNode::Element(run));
    element.children.push(XMLNode::Element(paragraph));
}

fn replace_first_text(element: &mut Element, value: &str) -> bool {
    for child in &mut element.children {
        match child {
            XMLNode::Element(child_element) => {
                if local_name(&child_element.name) == "t" {
                    if let Some(XMLNode::Text(text)) = child_element.children.iter_mut().find(|node| matches!(node, XMLNode::Text(_))) {
                        *text = value.to_string();
                        return true;
                    }
                }

                if replace_first_text(child_element, value) {
                    return true;
                }
            }
            XMLNode::Text(text) => {
                *text = value.to_string();
                return true;
            }
            _ => {}
        }
    }

    false
}

fn clone_with_text(template: &Element, value: &str) -> Element {
    let mut cloned = template.clone();
    set_first_text(&mut cloned, value);
    cloned
}

fn child_elements<'a>(element: &'a Element, name: &str) -> Vec<&'a Element> {
    element
        .children
        .iter()
        .filter_map(|child| match child {
            XMLNode::Element(child_element) if local_name(&child_element.name) == name => Some(child_element),
            _ => None,
        })
        .collect()
}

fn child_elements_mut<'a>(element: &'a mut Element, name: &str) -> Vec<&'a mut Element> {
    element
        .children
        .iter_mut()
        .filter_map(|child| match child {
            XMLNode::Element(child_element) if local_name(&child_element.name) == name => Some(child_element),
            _ => None,
        })
        .collect()
}

fn child_elements_count(element: &Element, name: &str) -> usize {
    child_elements(element, name).len()
}

fn remove_last_child_element(element: &mut Element, name: &str) {
    if let Some(index) = element.children.iter().rposition(|child| matches!(child, XMLNode::Element(child_element) if local_name(&child_element.name) == name)) {
        element.children.remove(index);
    }
}

fn find_child_mut<'a>(element: &'a mut Element, name: &str) -> Option<&'a mut Element> {
    element.children.iter_mut().find_map(|child| match child {
        XMLNode::Element(child_element) if local_name(&child_element.name) == name => Some(child_element),
        _ => None,
    })
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn fallback_text(value: &str, fallback: &str) -> String {
    if is_missing_value(value) {
        fallback.to_string()
    } else {
        value.trim().to_string()
    }
}

fn is_missing_value(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed == "待补充" || trimmed == "待补充部门" || trimmed == "待补充姓名"
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn starts_with_numbered_item(value: &str) -> bool {
    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let mut seen_digit = false;

    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            seen_digit = true;
            continue;
        }

        if (ch == '、' || ch == '.' || ch == '）' || ch == ')') && seen_digit {
            return true;
        }

        break;
    }

    false
}

fn normalize_member_name(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('【')
        .trim_end_matches('】')
        .trim_end_matches("（未发言）")
        .trim_end_matches("(未发言)")
        .trim()
        .to_string()
}

fn synthesize_summary(block: &SpeechBlock) -> Vec<String> {
    let mut items = Vec::new();

    if !block.weekly_summary.is_empty() {
        items.push(format!(
            "已完成{}项上周工作：{}",
            block.weekly_summary.len(),
            join_summary_items(&block.weekly_summary)
        ));
    }

    if !block.next_week_plan.is_empty() {
        items.push(format!(
            "本周重点推进{}项事项：{}",
            block.next_week_plan.len(),
            join_summary_items(&block.next_week_plan)
        ));
    }

    items
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

fn join_summary_items(items: &[String]) -> String {
    items.iter()
        .map(|item| trim_numbered_prefix(item).trim_end_matches('。').trim_end_matches('；').trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join("；")
}

fn trim_numbered_prefix(value: &str) -> &str {
    let trimmed = value.trim();
    let mut byte_index = 0usize;
    let mut seen_digit = false;

    for (index, ch) in trimmed.char_indices() {
        if ch.is_ascii_digit() {
            seen_digit = true;
            byte_index = index + ch.len_utf8();
            continue;
        }

        if seen_digit && (ch == '、' || ch == '.' || ch == '）' || ch == ')') {
            return trimmed[index + ch.len_utf8()..].trim();
        }

        break;
    }

    &trimmed[byte_index..].trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::{SystemTime, UNIX_EPOCH}};

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
        let xml = std::str::from_utf8(
            zip::ZipArchive::new(Cursor::new(TEMPLATE_DOCX_BYTES))
                .unwrap()
                .by_name("word/document.xml")
                .unwrap()
                .bytes()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .as_slice(),
        )
        .unwrap()
        .to_string();

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
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
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
