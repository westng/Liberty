use serde::Deserialize;
use serde_json::Value;

use crate::local_db::{AiSummaryActionItem, AiSummaryResult, LocalResult};

pub(super) fn parse_ai_summary_result(
    raw_response: &str,
    fallback_title: &str,
) -> LocalResult<AiSummaryResult> {
    let payload: OpenAiChatCompletionResponse = serde_json::from_str(raw_response)
        .map_err(|_| "AI 返回的原始 JSON 无法解析。".to_string())?;
    let content = extract_response_text(&payload)?;
    let structured: Value = serde_json::from_str(&content)
        .map_err(|_| "AI 返回的结构化 JSON 无法解析。".to_string())?;

    Ok(normalize_summary_result(&structured, fallback_title))
}

fn extract_response_text(payload: &OpenAiChatCompletionResponse) -> LocalResult<String> {
    let content = payload
        .choices
        .as_ref()
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.message.as_ref())
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

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionResponse {
    choices: Option<Vec<OpenAiChatCompletionChoice>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionChoice {
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
