use crate::local_db::{
    self, AiModelConfig, AiSummaryActionItem, AiSummaryResult, AiSummaryRun, AiSummaryTemplate,
    LocalResult, MeetingJob, MeetingMember, TranscriptSegment,
};
use reqwest::Client;
use reqwest::StatusCode;
use serde_json::Value;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tauri::AppHandle;

const AI_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const AI_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatCompletionInput {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatCompletionOutput {
    pub raw_response: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateAiSummaryInput {
    pub job: MeetingJob,
    pub model: AiModelConfig,
    pub template: AiSummaryTemplate,
    pub include_speaker: bool,
    pub include_timestamp: bool,
    #[serde(default)]
    pub use_member_mapping: bool,
    #[serde(default)]
    pub members: Vec<MeetingMember>,
    #[serde(default)]
    pub extra_instructions: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateAiSummaryOutput {
    pub prompt_preview: String,
    pub raw_response: String,
    pub result: AiSummaryResult,
}

#[tauri::command]
pub fn list_ai_models(app: AppHandle) -> LocalResult<Vec<AiModelConfig>> {
    local_db::list_ai_models(&app)
}

#[tauri::command]
pub fn save_ai_model(app: AppHandle, model: AiModelConfig) -> LocalResult<()> {
    local_db::save_ai_model(&app, &model)
}

#[tauri::command]
pub fn delete_ai_model(app: AppHandle, id: String) -> LocalResult<()> {
    local_db::delete_ai_model(&app, &id)
}

#[tauri::command]
pub fn list_ai_templates(app: AppHandle) -> LocalResult<Vec<AiSummaryTemplate>> {
    local_db::list_ai_templates(&app)
}

#[tauri::command]
pub fn save_ai_template(app: AppHandle, template: AiSummaryTemplate) -> LocalResult<()> {
    local_db::save_ai_template(&app, &template)
}

#[tauri::command]
pub fn delete_ai_template(app: AppHandle, id: String) -> LocalResult<()> {
    local_db::delete_ai_template(&app, &id)
}

#[tauri::command]
pub fn list_ai_summary_runs(app: AppHandle, job_id: String) -> LocalResult<Vec<AiSummaryRun>> {
    local_db::list_ai_summary_runs(&app, &job_id)
}

#[tauri::command]
pub fn save_ai_summary_run(app: AppHandle, run: AiSummaryRun) -> LocalResult<()> {
    local_db::save_ai_summary_run(&app, &run)
}

#[tauri::command]
pub fn set_active_ai_summary_run(app: AppHandle, job_id: String, run_id: String) -> LocalResult<()> {
    local_db::set_active_ai_summary_run(&app, &job_id, &run_id)
}

#[tauri::command]
pub fn delete_ai_summary_run(app: AppHandle, job_id: String, run_id: String) -> LocalResult<()> {
    local_db::delete_ai_summary_run(&app, &job_id, &run_id)
}

#[tauri::command]
pub fn request_ai_chat_completion(input: AiChatCompletionInput) -> LocalResult<AiChatCompletionOutput> {
    tauri::async_runtime::block_on(send_ai_chat_completion(input))
}

#[tauri::command]
pub async fn generate_ai_summary(input: GenerateAiSummaryInput) -> LocalResult<GenerateAiSummaryOutput> {
    let prompt_preview = build_summary_prompt_preview(&input);
    let completion = send_ai_chat_completion(AiChatCompletionInput {
        base_url: input.model.base_url.clone(),
        api_key: input.model.api_key.clone(),
        model: input.model.model.clone(),
        system_prompt: prompt_preview.system.clone(),
        user_prompt: prompt_preview.user.clone(),
    })
    .await?;

    let payload: OpenAiChatCompletionResponse = serde_json::from_str(&completion.raw_response)
        .map_err(|_| "AI 返回的原始 JSON 无法解析。".to_string())?;
    let content = extract_response_text(&payload)?;
    let structured: Value = serde_json::from_str(&content)
        .map_err(|_| "AI 返回的结构化 JSON 无法解析。".to_string())?;

    Ok(GenerateAiSummaryOutput {
        prompt_preview: format!("{}\n\n---\n\n{}", prompt_preview.system, prompt_preview.user),
        raw_response: completion.raw_response,
        result: normalize_summary_result(&structured, &input.job.title),
    })
}

async fn send_ai_chat_completion(input: AiChatCompletionInput) -> LocalResult<AiChatCompletionOutput> {
    let normalized_base_url = normalize_chat_base_url(&input.base_url);
    let normalized_model = normalize_model_id(&normalized_base_url, &input.model);
    if normalized_base_url.is_empty() {
        return Err("AI 接口地址不能为空。".into());
    }

    if input.api_key.trim().is_empty() {
        return Err("AI API Key 不能为空。".into());
    }

    if normalized_model.is_empty() {
        return Err("AI 模型名称不能为空。".into());
    }

    let client = Client::builder()
        .connect_timeout(AI_CONNECT_TIMEOUT)
        .timeout(AI_REQUEST_TIMEOUT)
        .build()
        .map_err(|err| format!("AI 请求客户端初始化失败: {err}"))?;

    let normalized_input = AiChatCompletionInput {
        model: normalized_model,
        ..input
    };

    match send_ai_chat_completion_once(&client, &normalized_input, &normalized_base_url, true).await {
        Ok(output) => Ok(output),
        Err(first_error) => {
            eprintln!(
                "[ai] request failed: base_url={} model={} include_response_format=true error={}",
                normalized_base_url,
                normalized_input.model,
                first_error.user_message()
            );
            if should_retry_without_response_format(&first_error) {
                return send_ai_chat_completion_once(&client, &normalized_input, &normalized_base_url, false)
                    .await
                    .map_err(|fallback_error| {
                        eprintln!(
                            "[ai] fallback request failed: base_url={} model={} include_response_format=false error={}",
                            normalized_base_url,
                            normalized_input.model,
                            fallback_error.user_message()
                        );
                        format!(
                            "{}；移除 response_format 兼容重试后仍失败：{}",
                            first_error.user_message(),
                            fallback_error.user_message()
                        )
                    });
            }

            if first_error.is_retryable_network() {
                return send_ai_chat_completion_once(&client, &normalized_input, &normalized_base_url, true)
                    .await
                    .map_err(|retry_error| {
                        eprintln!(
                            "[ai] retry request failed: base_url={} model={} include_response_format=true error={}",
                            normalized_base_url,
                            normalized_input.model,
                            retry_error.user_message()
                        );
                        format!(
                            "{}；自动重试 1 次后仍失败：{}",
                            first_error.user_message(),
                            retry_error.user_message()
                        )
                    });
            }

            Err(first_error.user_message())
        }
    }
}

async fn send_ai_chat_completion_once(
    client: &Client,
    input: &AiChatCompletionInput,
    normalized_base_url: &str,
    include_response_format: bool,
) -> Result<AiChatCompletionOutput, AiRequestError> {
    let mut payload = json!({
        "model": input.model.trim(),
        "messages": [
            {
                "role": "system",
                "content": input.system_prompt,
            },
            {
                "role": "user",
                "content": input.user_prompt,
            }
        ]
    });

    if include_response_format {
        payload["response_format"] = json!({ "type": "json_object" });
    }

    let response = client
        .post(format!("{normalized_base_url}/chat/completions"))
        .bearer_auth(input.api_key.trim())
        .json(&payload)
        .send()
        .await
        .map_err(AiRequestError::from_reqwest)?;

    let status = response.status();
    let raw_response = response
        .text()
        .await
        .map_err(AiRequestError::from_reqwest)?;

    if !status.is_success() {
        return Err(AiRequestError::http(
            status,
            raw_response,
            normalized_base_url.to_string(),
            include_response_format,
        ));
    }

    Ok(AiChatCompletionOutput { raw_response })
}

fn normalize_chat_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    trimmed
        .strip_suffix("/chat/completions")
        .unwrap_or(trimmed)
        .trim_end_matches('/')
        .to_string()
}

fn normalize_model_id(base_url: &str, model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if is_deepseek_base_url(base_url) {
        let normalized = trimmed
            .to_lowercase()
            .replace(['_', ' '], "-");
        return match normalized.as_str() {
            "deepseek-v4-flash" => "deepseek-v4-flash".into(),
            "deepseek-v4-pro" => "deepseek-v4-pro".into(),
            "deepseek-chat" => "deepseek-chat".into(),
            "deepseek-reasoner" => "deepseek-reasoner".into(),
            "deepseek-v4" => "deepseek-v4-pro".into(),
            _ => trimmed.to_string(),
        };
    }

    trimmed.to_string()
}

fn is_deepseek_base_url(base_url: &str) -> bool {
    let normalized = base_url.to_lowercase();
    normalized.contains("api.deepseek.com")
}

fn should_retry_without_response_format(error: &AiRequestError) -> bool {
    match error {
        AiRequestError::Http {
            status,
            body,
            include_response_format,
            ..
        } => {
            if !include_response_format {
                return false;
            }

            matches!(
                *status,
                StatusCode::BAD_REQUEST
                    | StatusCode::NOT_FOUND
                    | StatusCode::UNPROCESSABLE_ENTITY
                    | StatusCode::UNSUPPORTED_MEDIA_TYPE
            ) && mentions_response_format(body)
        }
        _ => false,
    }
}

fn mentions_response_format(body: &str) -> bool {
    let normalized = body.to_lowercase();
    normalized.contains("response_format")
        || normalized.contains("json_object")
        || normalized.contains("unsupported parameter")
        || normalized.contains("unknown parameter")
        || normalized.contains("invalid parameter")
}

#[derive(Debug)]
enum AiRequestError {
    Network(reqwest::Error),
    Http {
        status: StatusCode,
        body: String,
        base_url: String,
        include_response_format: bool,
    },
}

impl AiRequestError {
    fn from_reqwest(error: reqwest::Error) -> Self {
        Self::Network(error)
    }

    fn http(
        status: StatusCode,
        body: String,
        base_url: String,
        include_response_format: bool,
    ) -> Self {
        Self::Http {
            status,
            body,
            base_url,
            include_response_format,
        }
    }

    fn is_retryable_network(&self) -> bool {
        match self {
            Self::Network(error) => error.is_timeout() || error.is_connect() || error.is_request(),
            _ => false,
        }
    }

    fn user_message(&self) -> String {
        match self {
            Self::Network(error) => {
                if error.is_timeout() {
                    format!(
                        "AI 请求超时（{} 秒）。如果模型响应较慢或网络依赖代理，这类总结请求很容易触发超时。",
                        AI_REQUEST_TIMEOUT.as_secs()
                    )
                } else if error.is_connect() {
                    format!("AI 连接建立失败: {error}")
                } else if error.is_request() {
                    format!("AI 请求未发送成功: {error}")
                } else {
                    format!("AI 网络请求失败: {error}")
                }
            }
            Self::Http {
                status,
                body,
                base_url,
                ..
            } => {
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    if base_url.contains("api.deepseek.com")
                        && (trimmed.contains("supported API model names")
                            || trimmed.contains("but you passed"))
                    {
                        return format!(
                            "AI 接口请求失败，HTTP {status}: {trimmed}。模型字段必须填写 API model id，例如 deepseek-v4-flash，而不是展示名称。"
                        );
                    }

                    return format!("AI 接口请求失败，HTTP {status}: {trimmed}");
                }

                if *status == StatusCode::NOT_FOUND && !base_url.contains("/v1") {
                    return format!(
                        "AI 接口请求失败，HTTP {status}。请检查 Base URL 是否需要以 /v1 结尾，例如 OpenAI 兼容接口通常应填到 .../v1。"
                    );
                }

                format!("AI 接口请求失败，HTTP {status}")
            }
        }
    }
}

#[derive(Debug)]
struct PromptPreview {
    system: String,
    user: String,
}

fn build_summary_prompt_preview(input: &GenerateAiSummaryInput) -> PromptPreview {
    let transcript = primary_transcript_segments(&input.job)
        .iter()
        .map(|segment| format_segment(segment, input.include_speaker, input.include_timestamp))
        .collect::<Vec<_>>()
        .join("\n");

    let mut lines = vec![
        format!("Meeting title: {}", input.job.title),
        format!("Meeting language: {}", input.job.lang),
        format!(
            "Hotwords: {}",
            if input.job.hotwords.is_empty() {
                "none".to_string()
            } else {
                input.job.hotwords.join(", ")
            }
        ),
        format!("Include speaker info: {}", if input.include_speaker { "yes" } else { "no" }),
        format!("Include timestamps: {}", if input.include_timestamp { "yes" } else { "no" }),
        format!(
            "Use member mapping: {}",
            if input.use_member_mapping { "yes" } else { "no" }
        ),
        format!(
            "Extra instructions: {}",
            non_empty(&input.extra_instructions).unwrap_or("none")
        ),
    ];

    if input.use_member_mapping {
        lines.push(String::new());
        lines.push("Member directory mapping:".into());
        if input.members.is_empty() {
            lines.push("- No member directory records available.".into());
        } else {
            let mut members = input.members.clone();
            members.sort_by_key(|member| member.sort_order);
            lines.extend(members.into_iter().map(|member| {
                format!(
                    "- {} | department={} | sortOrder={} | recorder={}",
                    member.name,
                    if member.department.trim().is_empty() {
                        "未设置"
                    } else {
                        member.department.trim()
                    },
                    member.sort_order,
                    if member.is_recorder { "yes" } else { "no" }
                )
            }));
        }
        lines.push(String::new());
        lines.push("When the transcript already contains speaker names, keep those names exactly as they appear and use the member directory only to补充部门、排序相关上下文，不要改写姓名。".into());
    }

    lines.push(String::new());
    lines.push("Please output JSON based on the following meeting content:".into());
    lines.push(if transcript.trim().is_empty() {
        "Transcript is missing.".into()
    } else {
        transcript
    });

    PromptPreview {
        system: input.template.prompt.trim().to_string(),
        user: lines.join("\n"),
    }
}

fn primary_transcript_segments(job: &MeetingJob) -> &[TranscriptSegment] {
    if job.enable_speaker && !job.speaker_segments.is_empty() {
        &job.speaker_segments
    } else {
        &job.transcript_segments
    }
}

fn format_timestamp(ms: u64) -> String {
    let total_seconds = (ms / 1000) % 86_400;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn format_segment(segment: &TranscriptSegment, include_speaker: bool, include_timestamp: bool) -> String {
    let mut parts = Vec::new();
    if include_timestamp {
        parts.push(format!(
            "[{} - {}]",
            format_timestamp(segment.start_ms),
            format_timestamp(segment.end_ms)
        ));
    }
    if include_speaker {
        parts.push(format!(
            "{}:",
            segment
                .speaker
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Unknown speaker")
        ));
    }
    parts.push(segment.text.trim().to_string());
    parts.join(" ")
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
                items.iter()
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
