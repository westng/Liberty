mod client;
mod prompt;
mod response;
mod summary_runs;

use crate::local_db::{
    self, AiModelMetadata, AiModelSaveInput, AiSummaryRun, AiSummaryTemplate, LocalResult,
    MeetingJob, MeetingMember,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, WebviewWindow};

struct AiChatCompletionInput {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
}

struct AiChatCompletionOutput {
    pub raw_response: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryModelOption {
    pub id: String,
    pub name: String,
    pub model: String,
    pub enabled: bool,
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryOptions {
    pub models: Vec<AiSummaryModelOption>,
    pub templates: Vec<AiSummaryTemplate>,
}

struct GenerateAiSummaryInput {
    pub job: MeetingJob,
    pub template: AiSummaryTemplate,
    pub include_speaker: bool,
    pub include_timestamp: bool,
    pub use_member_mapping: bool,
    pub members: Vec<MeetingMember>,
    pub extra_instructions: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAiSummaryRunInput {
    pub source: String,
    pub job_id: String,
    #[serde(default)]
    pub window_scope_token: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub model_config_id: String,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub include_speaker: bool,
    #[serde(default)]
    pub include_timestamp: bool,
    #[serde(default)]
    pub use_member_mapping: bool,
    #[serde(default)]
    pub extra_instructions: String,
}

#[tauri::command]
pub fn list_ai_models(
    app: AppHandle,
    window: tauri::WebviewWindow,
) -> LocalResult<Vec<AiModelMetadata>> {
    require_model_management_window(window.label())?;
    local_db::list_ai_models(&app)
}

#[tauri::command]
pub fn save_ai_model(
    app: AppHandle,
    window: tauri::WebviewWindow,
    model: AiModelSaveInput,
) -> LocalResult<()> {
    require_model_management_window(window.label())?;
    local_db::save_ai_model(&app, &model)
}

#[tauri::command]
pub fn delete_ai_model(
    app: AppHandle,
    window: tauri::WebviewWindow,
    id: String,
) -> LocalResult<()> {
    require_model_management_window(window.label())?;
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
pub fn list_ai_summary_runs(
    app: AppHandle,
    window: WebviewWindow,
    source: String,
    job_id: String,
    window_scope_token: Option<String>,
) -> LocalResult<Vec<AiSummaryRun>> {
    require_local_source(&source)?;
    let job_id = normalize_job_id(&job_id)?;
    authorize_ai_summary_job(&window, job_id, window_scope_token.as_deref())?;
    local_db::list_ai_summary_runs(&app, job_id)
}

#[tauri::command]
pub fn get_ai_summary_options(app: AppHandle, source: String) -> LocalResult<AiSummaryOptions> {
    require_local_source(&source)?;
    local_db::init_database(&app)?;
    let conn = local_db::open_connection(&app)?;
    let models = crate::infrastructure::repositories::ai_models::list_ai_model_options(&conn)?
        .into_iter()
        .map(|model| AiSummaryModelOption {
            id: model.id,
            name: model.name,
            model: model.model,
            enabled: model.enabled,
            is_default: model.is_default,
        })
        .collect();
    let templates = crate::infrastructure::repositories::ai_templates::list_ai_templates(&conn)?;
    Ok(AiSummaryOptions { models, templates })
}

#[tauri::command]
pub fn set_active_ai_summary_run(
    app: AppHandle,
    window: WebviewWindow,
    source: String,
    job_id: String,
    run_id: String,
    window_scope_token: Option<String>,
) -> LocalResult<()> {
    require_local_source(&source)?;
    let job_id = normalize_job_id(&job_id)?;
    authorize_ai_summary_job(&window, job_id, window_scope_token.as_deref())?;
    let run_id = normalize_summary_run_id(&run_id)?;
    local_db::set_active_ai_summary_run(&app, job_id, run_id)
}

#[tauri::command]
pub fn delete_ai_summary_run(
    app: AppHandle,
    window: WebviewWindow,
    source: String,
    job_id: String,
    run_id: String,
    window_scope_token: Option<String>,
) -> LocalResult<()> {
    require_local_source(&source)?;
    let job_id = normalize_job_id(&job_id)?;
    authorize_ai_summary_job(&window, job_id, window_scope_token.as_deref())?;
    let run_id = normalize_summary_run_id(&run_id)?;
    local_db::delete_ai_summary_run(&app, job_id, run_id)
}

#[tauri::command]
pub fn start_or_resume_ai_summary_run(
    app: AppHandle,
    window: WebviewWindow,
    mut input: StartAiSummaryRunInput,
) -> LocalResult<AiSummaryRun> {
    require_local_source(&input.source)?;
    input.source = input.source.trim().to_string();
    input.job_id = normalize_job_id(&input.job_id)?.to_string();
    authorize_ai_summary_job(&window, &input.job_id, input.window_scope_token.as_deref())?;
    input.run_id = input
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|run_id| !run_id.is_empty())
        .map(|run_id| -> LocalResult<String> {
            validate_summary_run_id(run_id)?;
            Ok(run_id.to_string())
        })
        .transpose()?;
    input.model_config_id = input.model_config_id.trim().to_string();
    input.template_id = input.template_id.trim().to_string();
    summary_runs::start_or_resume(&app, input)
}

pub fn resume_ai_summary_runs(app: &AppHandle) -> LocalResult<()> {
    summary_runs::resume_running_on_startup(app)
}

fn require_local_source(source: &str) -> LocalResult<()> {
    if source.trim() == "local" {
        Ok(())
    } else {
        Err("远端 AI 总结能力尚未接入。".into())
    }
}

fn require_model_management_window(window_label: &str) -> LocalResult<()> {
    match window_label {
        "main" | "model-editor" => Ok(()),
        _ => Err("当前窗口无权读取或修改 AI 模型配置。".into()),
    }
}

fn authorize_ai_summary_job(
    window: &WebviewWindow,
    job_id: &str,
    scope_token: Option<&str>,
) -> LocalResult<()> {
    crate::window_scope::authorize_job_window(
        window,
        &[crate::window_scope::ai_summary_window()],
        job_id,
        scope_token,
    )
}

fn normalize_job_id(job_id: &str) -> LocalResult<&str> {
    let job_id = job_id.trim();
    crate::infrastructure::runner_files::validate_job_id(job_id)?;
    Ok(job_id)
}

fn normalize_summary_run_id(run_id: &str) -> LocalResult<&str> {
    let run_id = run_id.trim();
    validate_summary_run_id(run_id)?;
    Ok(run_id)
}

fn validate_summary_run_id(run_id: &str) -> LocalResult<()> {
    if run_id.len() > 80 || run_id.trim() != run_id {
        return Err("AI 总结运行 ID 格式无效。".into());
    }

    let valid_generated_id = run_id
        .strip_prefix("summary-run-")
        .and_then(|suffix| suffix.split_once('-'))
        .is_some_and(|(timestamp, sequence)| {
            timestamp.len() == 13
                && timestamp.bytes().all(|byte| byte.is_ascii_digit())
                && !sequence.is_empty()
                && sequence.len() <= 20
                && sequence.bytes().all(|byte| byte.is_ascii_digit())
        });
    if valid_generated_id || is_legacy_uuid(run_id) {
        Ok(())
    } else {
        Err("AI 总结运行 ID 格式无效。".into())
    }
}

fn is_legacy_uuid(value: &str) -> bool {
    const GROUP_LENGTHS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut groups = value.split('-');
    GROUP_LENGTHS.iter().all(|expected_length| {
        groups.next().is_some_and(|group| {
            group.len() == *expected_length && group.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    }) && groups.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_job_id, normalize_summary_run_id, require_local_source,
        require_model_management_window,
    };

    #[test]
    fn summary_command_identifiers_are_trimmed_and_validated() {
        assert_eq!(
            normalize_job_id(" job-1700000000000-0 ").expect("job id"),
            "job-1700000000000-0"
        );
        assert_eq!(
            normalize_summary_run_id(" summary-run-1700000000000-12 ").expect("generated run id"),
            "summary-run-1700000000000-12"
        );
        assert!(normalize_summary_run_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(normalize_summary_run_id("../summary-run-1700000000000-1").is_err());
        assert!(normalize_summary_run_id("summary-run-invalid-1").is_err());
        assert!(normalize_job_id("not-a-job").is_err());
    }

    #[test]
    fn local_source_is_trimmed_without_accepting_remote_fallback() {
        assert!(require_local_source(" local ").is_ok());
        assert!(require_local_source("remote").is_err());
        assert!(require_local_source("").is_err());
    }

    #[test]
    fn model_management_commands_are_restricted_by_window_label() {
        assert!(require_model_management_window("main").is_ok());
        assert!(require_model_management_window("model-editor").is_ok());
        assert!(require_model_management_window("ai-summary").is_err());
        assert!(require_model_management_window("meeting-notes").is_err());
    }
}
