use crate::local_export::model::{ExportDocData, SpeechBlock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeechSection {
    WeeklySummary,
    NextWeekPlan,
    Summary,
}

pub fn parse_overview_to_export_data(overview: &str) -> ExportDocData {
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
            if let Some(value) = strip_labeled_value(line, "会议名称") {
                data.meeting_name = value.trim().to_string();
            } else if let Some(value) = strip_labeled_value(line, "会议时间") {
                data.meeting_time = value.trim().to_string();
            } else if let Some(value) = strip_labeled_value(line, "会议地点") {
                data.meeting_location = value.trim().to_string();
            } else if let Some(value) = strip_labeled_value(line, "记录人") {
                data.recorder = value.trim().to_string();
            } else if let Some(value) = strip_labeled_value(line, "出席人员") {
                data.attendees = value.trim().to_string();
            } else if let Some(value) = strip_labeled_value(line, "缺席人员") {
                data.absentees = value.trim().to_string();
            } else if let Some(value) = strip_labeled_value(line, "主要议题") {
                data.topics = value.trim().to_string();
            } else if let Some(value) = strip_labeled_value(line, "会议主持人") {
                data.host = value.trim().to_string();
            } else if let Some(value) = strip_labeled_value(line, "审阅") {
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

        match trim_section_label(line) {
            "上周总结" => {
                current_section = Some(SpeechSection::WeeklySummary);
                continue;
            }
            "本周计划" => {
                current_section = Some(SpeechSection::NextWeekPlan);
                continue;
            }
            "个人总结" => {
                current_section = Some(SpeechSection::Summary);
                in_closing_summary = false;
                continue;
            }
            "全局总结" => {
                if let Some(block) = current_block.take() {
                    data.speech_blocks.push(block);
                }
                current_section = None;
                in_closing_summary = true;
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

fn strip_labeled_value<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    line.trim()
        .strip_prefix(label)
        .and_then(|value| value.trim_start().strip_prefix(['：', ':']))
}

fn trim_section_label(line: &str) -> &str {
    line.trim().trim_end_matches(['：', ':']).trim()
}

pub fn parse_speaker_header(line: &str) -> Option<(String, String)> {
    let original = line.trim();
    if starts_with_numbered_item(original) {
        return None;
    }

    let normalized = original.replace('：', ":");
    let (left, right) = normalized.split_once(':')?;
    let raw_department = left.trim();
    let department = left
        .trim()
        .trim_start_matches('【')
        .trim_end_matches('】')
        .trim();
    let name = right
        .trim()
        .trim_start_matches('【')
        .trim_end_matches('】')
        .trim();

    if department.is_empty() || name.is_empty() {
        return None;
    }

    if !is_bracketed_label(raw_department) && !looks_like_department_label(department) {
        return None;
    }

    Some((department.to_string(), name.to_string()))
}

fn is_bracketed_label(value: &str) -> bool {
    value.starts_with('【') && value.ends_with('】')
}

fn looks_like_department_label(value: &str) -> bool {
    [
        "部", "办", "室", "中心", "组", "科", "处", "局", "院", "园", "厅", "店", "前台", "客房",
        "温泉", "营销", "财务", "行政", "人事", "工程", "保安", "餐饮",
    ]
    .iter()
    .any(|keyword| value.contains(keyword))
}

pub fn is_missing_value(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed == "待补充" || trimmed == "待补充部门" || trimmed == "待补充姓名"
}

pub fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn starts_with_numbered_item(value: &str) -> bool {
    let trimmed = value.trim();
    let mut seen_digit = false;

    for ch in trimmed.chars() {
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

pub fn normalize_member_name(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('【')
        .trim_end_matches('】')
        .trim_end_matches("（未发言）")
        .trim_end_matches("(未发言)")
        .trim()
        .to_string()
}

pub fn trim_numbered_prefix(value: &str) -> &str {
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

    trimmed[byte_index..].trim()
}
