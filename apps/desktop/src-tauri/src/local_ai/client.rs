use reqwest::{Client, StatusCode};
use serde_json::json;
use std::time::Duration;

use crate::local_ai::prompt::{
    REQUIRED_SPEAKERS_PREFIX, TRANSCRIPT_END_MARKER, TRANSCRIPT_START_MARKER,
};
use crate::local_ai::{AiChatCompletionInput, AiChatCompletionOutput};
use crate::local_db::LocalResult;

const AI_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const AI_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_SUMMARY_INPUT_TOKENS: usize = 12_000;

pub(super) async fn send_ai_chat_completion_chunk(
    input: AiChatCompletionInput,
) -> LocalResult<AiChatCompletionOutput> {
    let (client, normalized_input, normalized_base_url) = prepare_request(input)?;
    send_ai_chat_completion_with_retry(&client, &normalized_input, &normalized_base_url).await
}

fn prepare_request(
    input: AiChatCompletionInput,
) -> LocalResult<(Client, AiChatCompletionInput, String)> {
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
    Ok((client, normalized_input, normalized_base_url))
}

async fn send_ai_chat_completion_with_retry(
    client: &Client,
    input: &AiChatCompletionInput,
    normalized_base_url: &str,
) -> LocalResult<AiChatCompletionOutput> {
    match send_ai_chat_completion_once(client, input, normalized_base_url, true).await {
        Ok(output) => Ok(output),
        Err(first_error) => {
            handle_first_request_error(client, input, normalized_base_url, first_error).await
        }
    }
}

async fn handle_first_request_error(
    client: &Client,
    input: &AiChatCompletionInput,
    normalized_base_url: &str,
    first_error: AiRequestError,
) -> LocalResult<AiChatCompletionOutput> {
    eprintln!(
        "[ai] request failed: base_url={} model={} include_response_format=true error={}",
        normalized_base_url,
        input.model,
        first_error.user_message()
    );

    if should_retry_without_response_format(&first_error) {
        return send_ai_chat_completion_once(client, input, normalized_base_url, false)
            .await
            .map_err(|fallback_error| {
                eprintln!(
                    "[ai] fallback request failed: base_url={} model={} include_response_format=false error={}",
                    normalized_base_url,
                    input.model,
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
        return send_ai_chat_completion_once(client, input, normalized_base_url, true)
            .await
            .map_err(|retry_error| {
                eprintln!(
                    "[ai] retry request failed: base_url={} model={} include_response_format=true error={}",
                    normalized_base_url,
                    input.model,
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

pub(super) struct SummaryRequestChunks {
    pub fallback_title: String,
    pub required_speakers: Vec<String>,
    pub user_prompts: Vec<String>,
}

pub(super) fn plan_summary_request(input: &AiChatCompletionInput) -> Option<SummaryRequestChunks> {
    let (prefix, remainder) = input.user_prompt.split_once(TRANSCRIPT_START_MARKER)?;
    let (transcript, suffix) = remainder.split_once(TRANSCRIPT_END_MARKER)?;
    let required_speakers = prefix
        .lines()
        .find_map(|line| line.trim().strip_prefix(REQUIRED_SPEAKERS_PREFIX))
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    let fallback_title = prefix
        .lines()
        .find_map(|line| line.trim().strip_prefix("Meeting title: "))
        .unwrap_or("会议总结")
        .trim()
        .to_string();
    let fixed_tokens = estimate_tokens(&input.system_prompt)
        + estimate_tokens(prefix)
        + estimate_tokens(suffix)
        + 512;
    let transcript_budget = MAX_SUMMARY_INPUT_TOKENS
        .saturating_sub(fixed_tokens)
        .max(1_000);
    let chunks = chunk_transcript(transcript.trim(), transcript_budget);
    let total = chunks.len();
    let user_prompts = chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| {
            format!(
                "{}\n{}\nSummary chunk {}/{}. Summarize only this chunk; keep all required JSON fields.\n{}\n{}{}",
                prefix.trim_end(),
                TRANSCRIPT_START_MARKER,
                index + 1,
                total,
                chunk,
                TRANSCRIPT_END_MARKER,
                suffix
            )
        })
        .collect();

    Some(SummaryRequestChunks {
        fallback_title,
        required_speakers,
        user_prompts,
    })
}

fn chunk_transcript(transcript: &str, token_budget: usize) -> Vec<String> {
    if transcript.trim().is_empty() {
        return vec!["Transcript is missing.".into()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_tokens = 0usize;
    for line in transcript
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let line_tokens = estimate_tokens(line).max(1);
        if !current.is_empty() && current_tokens + line_tokens > token_budget {
            chunks.push(std::mem::take(&mut current));
            current_tokens = 0;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
        current_tokens += line_tokens;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn estimate_tokens(value: &str) -> usize {
    let mut ascii = 0usize;
    let mut non_ascii = 0usize;
    for character in value.chars() {
        if character.is_ascii() {
            ascii += 1;
        } else {
            non_ascii += 1;
        }
    }
    ascii.div_ceil(4) + non_ascii
}

fn normalize_model_id(base_url: &str, model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if is_deepseek_base_url(base_url) {
        let normalized = trimmed.to_lowercase().replace(['_', ' '], "-");
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
            Self::Network(error) => network_error_message(error),
            Self::Http {
                status,
                body,
                base_url,
                ..
            } => http_error_message(*status, body, base_url),
        }
    }
}

fn network_error_message(error: &reqwest::Error) -> String {
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

fn http_error_message(status: StatusCode, body: &str, base_url: &str) -> String {
    let trimmed = body.trim();
    if !trimmed.is_empty() {
        if base_url.contains("api.deepseek.com")
            && (trimmed.contains("supported API model names") || trimmed.contains("but you passed"))
        {
            return format!(
                "AI 接口请求失败，HTTP {status}: {trimmed}。模型字段必须填写 API model id，例如 deepseek-v4-flash，而不是展示名称。"
            );
        }

        return format!("AI 接口请求失败，HTTP {status}: {trimmed}");
    }

    if status == StatusCode::NOT_FOUND && !base_url.contains("/v1") {
        return format!(
            "AI 接口请求失败，HTTP {status}。请检查 Base URL 是否需要以 /v1 结尾，例如 OpenAI 兼容接口通常应填到 .../v1。"
        );
    }

    format!("AI 接口请求失败，HTTP {status}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_transcript_splits_only_between_lines() {
        let transcript = (0..40)
            .map(|index| format!("speaker-{index}: {}", "内容".repeat(40)))
            .collect::<Vec<_>>()
            .join("\n");

        let chunks = chunk_transcript(&transcript, 180);

        assert!(chunks.len() > 1);
        assert_eq!(chunks.join("\n"), transcript);
        assert!(chunks.iter().all(|chunk| !chunk.starts_with("内容")));
    }

    #[test]
    fn summary_request_preserves_required_speakers_in_every_chunk() {
        let input = AiChatCompletionInput {
            base_url: "https://example.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            user_prompt: format!(
                "Meeting title: 周会\n{REQUIRED_SPEAKERS_PREFIX}[\"李兰\",\"肖明容\"]\n{TRANSCRIPT_START_MARKER}\n{}\n{TRANSCRIPT_END_MARKER}",
                "李兰: 内容\n肖明容: 内容\n".repeat(2_000)
            ),
        };

        let request = plan_summary_request(&input).unwrap();

        assert!(request.user_prompts.len() > 1);
        assert_eq!(request.required_speakers, vec!["李兰", "肖明容"]);
        assert!(request
            .user_prompts
            .iter()
            .all(|prompt| prompt.contains(TRANSCRIPT_START_MARKER)));
    }
}
