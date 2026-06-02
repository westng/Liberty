mod client;
mod prompt;
mod response;

use crate::local_db::{
    self, AiModelConfig, AiSummaryResult, AiSummaryRun, AiSummaryTemplate, LocalResult, MeetingJob,
    MeetingMember, MeetingMinutesPayload,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use client::send_ai_chat_completion;
use prompt::build_summary_prompt_preview;
use response::parse_ai_summary_result;

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
    pub minutes_payload: MeetingMinutesPayload,
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
pub fn set_active_ai_summary_run(
    app: AppHandle,
    job_id: String,
    run_id: String,
) -> LocalResult<()> {
    local_db::set_active_ai_summary_run(&app, &job_id, &run_id)
}

#[tauri::command]
pub fn delete_ai_summary_run(app: AppHandle, job_id: String, run_id: String) -> LocalResult<()> {
    local_db::delete_ai_summary_run(&app, &job_id, &run_id)
}

#[tauri::command]
pub fn request_ai_chat_completion(
    input: AiChatCompletionInput,
) -> LocalResult<AiChatCompletionOutput> {
    tauri::async_runtime::block_on(send_ai_chat_completion(input))
}

#[tauri::command]
pub async fn generate_ai_summary(
    input: GenerateAiSummaryInput,
) -> LocalResult<GenerateAiSummaryOutput> {
    let prompt_preview = build_summary_prompt_preview(&input);
    let completion = send_ai_chat_completion(AiChatCompletionInput {
        base_url: input.model.base_url.clone(),
        api_key: input.model.api_key.clone(),
        model: input.model.model.clone(),
        system_prompt: prompt_preview.system.clone(),
        user_prompt: prompt_preview.user.clone(),
    })
    .await?;

    let result = parse_ai_summary_result(&completion.raw_response, &input.job.title)?;
    let minutes_payload = crate::local_export::derive_meeting_minutes_payload(
        &input.job,
        &result,
        &input.members,
        &input.template.id,
        None,
    );

    Ok(GenerateAiSummaryOutput {
        prompt_preview: format!(
            "{}\n\n---\n\n{}",
            prompt_preview.system, prompt_preview.user
        ),
        result,
        minutes_payload,
        raw_response: completion.raw_response,
    })
}
