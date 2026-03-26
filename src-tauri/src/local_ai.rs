use crate::local_db::{self, AiModelConfig, AiSummaryRun, AiSummaryTemplate, LocalResult};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::AppHandle;

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
    let normalized_base_url = input.base_url.trim().trim_end_matches('/');
    if normalized_base_url.is_empty() {
        return Err("AI 接口地址不能为空。".into());
    }

    if input.api_key.trim().is_empty() {
        return Err("AI API Key 不能为空。".into());
    }

    if input.model.trim().is_empty() {
        return Err("AI 模型名称不能为空。".into());
    }

    let client = Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| format!("AI 请求客户端初始化失败: {err}"))?;

    let response = client
        .post(format!("{normalized_base_url}/chat/completions"))
        .bearer_auth(input.api_key.trim())
        .json(&json!({
            "model": input.model.trim(),
            "response_format": { "type": "json_object" },
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
        }))
        .send()
        .map_err(|err| format!("AI 请求未发送成功: {err}"))?;

    let status = response.status();
    let raw_response = response
        .text()
        .map_err(|err| format!("AI 响应读取失败: {err}"))?;

    if !status.is_success() {
        return Err(if raw_response.trim().is_empty() {
            format!("AI 接口请求失败，HTTP {status}")
        } else {
            raw_response
        });
    }

    Ok(AiChatCompletionOutput { raw_response })
}
