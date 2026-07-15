use serde::Deserialize;
use serde_json::{json, Value};

use crate::local_db::{AiSummaryActionItem, AiSummaryResult, LocalResult};

const REQUIRED_FIELDS: [&str; 9] = [
    "title",
    "overview",
    "topics",
    "decisions",
    "actionItems",
    "risks",
    "followUps",
    "speakerReports",
    "globalSummary",
];

pub(super) fn parse_ai_summary_result(
    raw_response: &str,
    fallback_title: &str,
) -> LocalResult<AiSummaryResult> {
    let structured = parse_ai_summary_structured(raw_response)?;

    Ok(normalize_summary_result(&structured, fallback_title))
}

pub(super) fn parse_ai_summary_structured(raw_response: &str) -> LocalResult<Value> {
    let payload: OpenAiChatCompletionResponse = serde_json::from_str(raw_response)
        .map_err(|_| "AI 返回的原始 JSON 无法解析。".to_string())?;
    let structured = parse_structured_content(&payload)?;
    validate_required_fields(&structured)?;
    Ok(structured)
}

pub(super) fn merge_ai_summary_responses(
    raw_responses: &[String],
    fallback_title: &str,
    required_speakers: &[String],
) -> LocalResult<String> {
    if raw_responses.is_empty() {
        return Err("AI 分块结果为空。".into());
    }

    let mut merged = json!({
        "title": fallback_title,
        "overview": "",
        "topics": [],
        "decisions": [],
        "actionItems": [],
        "risks": [],
        "followUps": [],
        "speakerReports": [],
        "globalSummary": []
    });

    for raw_response in raw_responses {
        let payload: OpenAiChatCompletionResponse = serde_json::from_str(raw_response)
            .map_err(|_| "AI 返回的原始 JSON 无法解析。".to_string())?;
        let structured = parse_structured_content(&payload)?;
        validate_required_fields(&structured)?;
        merge_structured_summary(&mut merged, &structured);
    }

    ensure_required_speaker_reports(&mut merged, required_speakers);
    materialize_overview(&mut merged);
    validate_required_fields(&merged)?;

    let content =
        serde_json::to_string(&merged).map_err(|err| format!("AI 合并结果序列化失败: {err}"))?;
    serde_json::to_string(&json!({
        "choices": [{
            "finish_reason": "stop",
            "message": { "content": content }
        }]
    }))
    .map_err(|err| format!("AI 合并响应序列化失败: {err}"))
}

fn parse_structured_content(payload: &OpenAiChatCompletionResponse) -> LocalResult<Value> {
    let content = extract_response_text(payload)?;
    serde_json::from_str(&content).map_err(|_| "AI 返回的结构化 JSON 无法解析。".to_string())
}

fn extract_response_text(payload: &OpenAiChatCompletionResponse) -> LocalResult<String> {
    let choice = payload
        .choices
        .as_ref()
        .and_then(|choices| choices.first())
        .ok_or_else(|| "AI 响应 choices 为空。".to_string())?;
    validate_finish_reason(choice.finish_reason.as_deref())?;
    let content = choice
        .message
        .as_ref()
        .and_then(|message| message.content.as_ref())
        .ok_or_else(|| "AI 响应内容为空。".to_string())?;

    match content {
        ResponseContent::Text(text) => Ok(text.trim().to_string()),
        ResponseContent::Parts(parts) => Ok(parts
            .iter()
            .map(|part| part.text.as_deref().unwrap_or(""))
            .collect::<String>()
            .trim()
            .to_string()),
    }
}

fn validate_finish_reason(finish_reason: Option<&str>) -> LocalResult<()> {
    match finish_reason
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
    {
        Some("stop") => Ok(()),
        Some("length") => Err("AI 响应因长度限制被截断，请缩短内容或使用分块总结。".into()),
        Some(reason) => Err(format!("AI 响应未正常完成，finish_reason={reason}。")),
        None => Err("AI 响应缺少 finish_reason，无法确认结果完整。".into()),
    }
}

fn validate_required_fields(input: &Value) -> LocalResult<()> {
    let missing = REQUIRED_FIELDS
        .iter()
        .filter(|field| input.get(**field).is_none())
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "AI 结构化结果缺少必需字段: {}。",
            missing.join(", ")
        ))
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionResponse {
    choices: Option<Vec<OpenAiChatCompletionChoice>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionChoice {
    finish_reason: Option<String>,
    message: Option<OpenAiChatCompletionMessage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionMessage {
    content: Option<ResponseContent>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ResponseContent {
    Text(String),
    Parts(Vec<ResponseContentPart>),
}

#[derive(Debug, Deserialize)]
struct ResponseContentPart {
    text: Option<String>,
}

fn normalize_summary_result(input: &Value, fallback_title: &str) -> AiSummaryResult {
    AiSummaryResult {
        title: to_trimmed_string(input.get("title")).unwrap_or_else(|| fallback_title.to_string()),
        overview: to_trimmed_string(input.get("overview")).unwrap_or_default(),
        topics: to_string_array(input.get("topics")),
        decisions: to_string_array(input.get("decisions")),
        action_items: input
            .get("actionItems")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let task = to_trimmed_string(item.get("task")).unwrap_or_default();
                        if task.is_empty() {
                            return None;
                        }
                        Some(AiSummaryActionItem {
                            task,
                            owner: to_trimmed_string(item.get("owner")).unwrap_or_default(),
                            due_date: to_trimmed_string(item.get("dueDate")).unwrap_or_default(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        risks: to_string_array(input.get("risks")),
        follow_ups: to_string_array(input.get("followUps")),
    }
}

fn merge_structured_summary(target: &mut Value, source: &Value) {
    if target
        .get("title")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        target["title"] = source.get("title").cloned().unwrap_or(Value::Null);
    }

    append_unique_values(target, source, "overview");
    for field in [
        "topics",
        "decisions",
        "actionItems",
        "risks",
        "followUps",
        "globalSummary",
    ] {
        append_unique_array(target, source, field);
    }
    merge_speaker_reports(target, source);
}

fn append_unique_values(target: &mut Value, source: &Value, field: &str) {
    let Some(value) = to_trimmed_string(source.get(field)) else {
        return;
    };
    let current = target.get(field).and_then(Value::as_str).unwrap_or("");
    if current.is_empty() {
        target[field] = Value::String(value);
    } else if !current.lines().any(|line| line.trim() == value) {
        target[field] = Value::String(format!("{current}\n{value}"));
    }
}

fn append_unique_array(target: &mut Value, source: &Value, field: &str) {
    let Some(source_items) = source.get(field).and_then(Value::as_array) else {
        return;
    };
    let target_items = target[field]
        .as_array_mut()
        .expect("merged field is an array");
    for item in source_items {
        if !target_items.contains(item) {
            target_items.push(item.clone());
        }
    }
}

fn merge_speaker_reports(target: &mut Value, source: &Value) {
    let Some(source_reports) = source.get("speakerReports").and_then(Value::as_array) else {
        return;
    };
    let target_reports = target["speakerReports"]
        .as_array_mut()
        .expect("speakerReports is an array");
    for source_report in source_reports {
        let label = speaker_label(source_report);
        if label.is_empty() {
            continue;
        }
        if let Some(target_report) = target_reports
            .iter_mut()
            .find(|report| speaker_label(report) == label)
        {
            for field in ["weeklySummary", "nextWeekPlan", "summary"] {
                append_unique_array(target_report, source_report, field);
            }
            if target_report
                .get("department")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                target_report["department"] = source_report
                    .get("department")
                    .cloned()
                    .unwrap_or_else(|| Value::String(String::new()));
            }
        } else {
            target_reports.push(normalize_speaker_report(source_report, &label));
        }
    }
}

fn ensure_required_speaker_reports(target: &mut Value, required_speakers: &[String]) {
    let reports = target["speakerReports"]
        .as_array_mut()
        .expect("speakerReports is an array");
    for label in required_speakers {
        let label = label.trim();
        if label.is_empty() || reports.iter().any(|report| speaker_label(report) == label) {
            continue;
        }
        reports.push(json!({
            "speakerLabel": label,
            "department": "",
            "weeklySummary": [],
            "nextWeekPlan": [],
            "summary": [],
            "matchStatus": "missing_from_ai"
        }));
    }
}

fn materialize_overview(target: &mut Value) {
    let mut lines = Vec::new();
    if let Some(overview) = target.get("overview").and_then(Value::as_str) {
        lines.extend(
            overview
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string),
        );
    }
    let reports = target["speakerReports"]
        .as_array()
        .expect("speakerReports is an array");
    if !reports.is_empty() {
        lines.push("发言内容".into());
    }
    for report in reports {
        let label = speaker_label(report);
        if label.is_empty() {
            continue;
        }
        let department = report
            .get("department")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("未匹配部门");
        lines.push(format!("【{department}】：{label}"));
        append_overview_section(&mut lines, report, "上周总结", "weeklySummary");
        append_overview_section(&mut lines, report, "本周计划", "nextWeekPlan");
        append_overview_section(&mut lines, report, "个人总结", "summary");
    }
    let global_summary = target["globalSummary"]
        .as_array()
        .expect("globalSummary is an array");
    if !global_summary.is_empty() {
        lines.push("全局总结：".into());
        lines.extend(
            global_summary
                .iter()
                .filter_map(|item| to_trimmed_string(Some(item))),
        );
    }
    target["overview"] = Value::String(lines.join("\n"));
}

fn append_overview_section(lines: &mut Vec<String>, report: &Value, label: &str, field: &str) {
    lines.push(format!("{label}："));
    let items = report
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    lines.extend(
        items
            .iter()
            .filter_map(|item| to_trimmed_string(Some(item))),
    );
}

fn normalize_speaker_report(source: &Value, label: &str) -> Value {
    json!({
        "speakerLabel": label,
        "department": to_trimmed_string(source.get("department")).unwrap_or_default(),
        "weeklySummary": source.get("weeklySummary").and_then(Value::as_array).cloned().unwrap_or_default(),
        "nextWeekPlan": source.get("nextWeekPlan").and_then(Value::as_array).cloned().unwrap_or_default(),
        "summary": source.get("summary").and_then(Value::as_array).cloned().unwrap_or_default(),
        "matchStatus": source.get("matchStatus").cloned().unwrap_or_else(|| Value::String("unmatched".into()))
    })
}

fn speaker_label(report: &Value) -> String {
    to_trimmed_string(report.get("speakerLabel"))
        .or_else(|| to_trimmed_string(report.get("resolvedName")))
        .or_else(|| to_trimmed_string(report.get("name")))
        .unwrap_or_default()
}

fn to_trimmed_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(boolean)) => Some(boolean.to_string()),
        Some(Value::Array(items)) => {
            let joined = items
                .iter()
                .filter_map(|item| to_trimmed_string(Some(item)))
                .collect::<Vec<_>>()
                .join(" ");
            non_empty(&joined).map(str::to_string)
        }
        Some(Value::Object(map)) => {
            let joined = map
                .values()
                .filter_map(|item| to_trimmed_string(Some(item)))
                .collect::<Vec<_>>()
                .join(" ");
            non_empty(&joined).map(str::to_string)
        }
        _ => None,
    }
}

fn to_string_array(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| to_trimmed_string(Some(item)))
            .collect(),
        _ => to_trimmed_string(value).into_iter().collect(),
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(content: Value, finish_reason: Option<&str>) -> String {
        serde_json::to_string(&json!({
            "choices": [{
                "finish_reason": finish_reason,
                "message": { "content": serde_json::to_string(&content).unwrap() }
            }]
        }))
        .unwrap()
    }

    fn complete_summary(speaker: &str) -> Value {
        json!({
            "title": "周会",
            "overview": "阶段摘要",
            "topics": ["议题"],
            "decisions": [],
            "actionItems": [],
            "risks": [],
            "followUps": [],
            "speakerReports": [{
                "speakerLabel": speaker,
                "department": "营销部",
                "weeklySummary": [format!("{speaker}事项")],
                "nextWeekPlan": [],
                "summary": []
            }],
            "globalSummary": []
        })
    }

    #[test]
    fn truncated_response_is_rejected() {
        let error =
            parse_ai_summary_result(&response(complete_summary("李兰"), Some("length")), "周会")
                .unwrap_err();

        assert!(error.contains("截断"));
    }

    #[test]
    fn missing_finish_reason_is_rejected() {
        let error =
            parse_ai_summary_result(&response(complete_summary("李兰"), None), "周会").unwrap_err();

        assert!(error.contains("finish_reason"));
    }

    #[test]
    fn chunk_merge_deduplicates_and_backfills_missing_speakers() {
        let first = response(complete_summary("李兰"), Some("stop"));
        let second = response(complete_summary("李兰"), Some("stop"));

        let merged =
            merge_ai_summary_responses(&[first, second], "周会", &["李兰".into(), "肖明容".into()])
                .unwrap();
        let payload: OpenAiChatCompletionResponse = serde_json::from_str(&merged).unwrap();
        let structured = parse_structured_content(&payload).unwrap();
        let reports = structured["speakerReports"].as_array().unwrap();

        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0]["weeklySummary"].as_array().unwrap().len(), 1);
        assert_eq!(reports[1]["speakerLabel"], "肖明容");
        assert_eq!(reports[1]["matchStatus"], "missing_from_ai");
        assert!(structured["overview"].as_str().unwrap().contains("肖明容"));
    }
}
