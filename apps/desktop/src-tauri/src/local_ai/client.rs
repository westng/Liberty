use reqwest::{Client, StatusCode};
use serde_json::json;
use std::time::Duration;

use crate::local_ai::{AiChatCompletionInput, AiChatCompletionOutput};
use crate::local_db::LocalResult;

const AI_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const AI_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub(super) async fn send_ai_chat_completion(
    input: AiChatCompletionInput,
) -> LocalResult<AiChatCompletionOutput> {
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

    match send_ai_chat_completion_once(&client, &normalized_input, &normalized_base_url, true).await
    {
        Ok(output) => Ok(output),
        Err(first_error) => {
            handle_first_request_error(
                &client,
                &normalized_input,
                &normalized_base_url,
                first_error,
            )
            .await
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
