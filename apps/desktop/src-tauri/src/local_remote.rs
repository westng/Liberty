use std::{collections::HashSet, io::Read, time::Duration};

use reqwest::{
    blocking::{Client, Response},
    header::{ACCEPT, CONTENT_TYPE},
    Method,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, WebviewWindow};

use crate::{
    domain::{
        asr::{AsrBackend, DiarizationStatus, RunnerWarning},
        error::{AppErrorDto, ErrorCategory},
    },
    infrastructure::network::{TrustedHttpPolicy, TrustedHttpTarget},
    local_db::{self, LocalResult, MeetingJob},
};

const PROTOCOL_NAME: &str = "liberty-meeting";
const PROTOCOL_VERSION: u32 = 1;
const JOB_SCHEMA_VERSION: u32 = 1;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ERROR_BYTES: u64 = 64 * 1024;
const KNOWN_OPERATIONS: &[&str] = &[
    "jobs.list",
    "jobs.read",
    "jobs.result.read",
    "jobs.create",
    "jobs.retry",
    "jobs.delete",
    "transcript.speakers.rename",
    "summary.runs.read",
    "summary.runs.write",
    "exports.generate",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMeetingCapabilities {
    protocol: String,
    protocol_version: u32,
    job_schema_version: u32,
    service_version: String,
    operations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    job_create: Option<RemoteJobCreateCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exports: Option<RemoteExportCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteJobCreateCapability {
    upload_mode: String,
    max_files: u64,
    max_bytes_per_file: u64,
    extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteExportCapability {
    formats: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteErrorEnvelope {
    error: Option<RemoteErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct RemoteErrorDetail {
    code: Option<String>,
    message: Option<String>,
}

struct RemoteContext {
    client: Client,
    target: TrustedHttpTarget,
    api_token: String,
}

#[tauri::command]
pub fn get_remote_capabilities(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<RemoteMeetingCapabilities, AppErrorDto> {
    require_main_window(window.label()).map_err(remote_command_error)?;
    let context = remote_context(&app).map_err(remote_command_error)?;
    fetch_capabilities(&context).map_err(remote_command_error)
}

#[tauri::command]
pub fn remote_list_jobs(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Vec<MeetingJob>, AppErrorDto> {
    require_main_window(window.label()).map_err(remote_command_error)?;
    let context = remote_context(&app).map_err(remote_command_error)?;
    let capabilities = fetch_capabilities(&context).map_err(remote_command_error)?;
    require_operation(&capabilities, "jobs.list").map_err(remote_command_error)?;
    request_jobs(&context, Method::GET, &["api", "jobs"], None).map_err(remote_command_error)
}

#[tauri::command]
pub fn remote_get_job(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<MeetingJob, AppErrorDto> {
    require_main_window(window.label()).map_err(remote_command_error)?;
    let id = normalize_job_id(&id).map_err(remote_command_error)?;
    let context = remote_context(&app).map_err(remote_command_error)?;
    let capabilities = fetch_capabilities(&context).map_err(remote_command_error)?;
    require_operation(&capabilities, "jobs.read").map_err(remote_command_error)?;
    request_job(&context, Method::GET, &["api", "jobs", id], None, id).map_err(remote_command_error)
}

#[tauri::command]
pub fn remote_get_job_result(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<MeetingJob, AppErrorDto> {
    require_main_window(window.label()).map_err(remote_command_error)?;
    let id = normalize_job_id(&id).map_err(remote_command_error)?;
    let context = remote_context(&app).map_err(remote_command_error)?;
    let capabilities = fetch_capabilities(&context).map_err(remote_command_error)?;
    require_operation(&capabilities, "jobs.result.read").map_err(remote_command_error)?;
    request_job(
        &context,
        Method::GET,
        &["api", "jobs", id, "result"],
        None,
        id,
    )
    .map_err(remote_command_error)
}

#[tauri::command]
pub fn remote_retry_job(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<MeetingJob, AppErrorDto> {
    require_main_window(window.label()).map_err(remote_command_error)?;
    let id = normalize_job_id(&id).map_err(remote_command_error)?;
    let context = remote_context(&app).map_err(remote_command_error)?;
    let capabilities = fetch_capabilities(&context).map_err(remote_command_error)?;
    require_operation(&capabilities, "jobs.retry").map_err(remote_command_error)?;
    request_job(
        &context,
        Method::POST,
        &["api", "jobs", id, "retry"],
        None,
        id,
    )
    .map_err(remote_command_error)
}

#[tauri::command]
pub fn remote_delete_job(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<(), AppErrorDto> {
    require_main_window(window.label()).map_err(remote_command_error)?;
    let id = normalize_job_id(&id).map_err(remote_command_error)?;
    let context = remote_context(&app).map_err(remote_command_error)?;
    let capabilities = fetch_capabilities(&context).map_err(remote_command_error)?;
    require_operation(&capabilities, "jobs.delete").map_err(remote_command_error)?;
    let response = send_request(&context, Method::DELETE, &["api", "jobs", id], None)
        .map_err(remote_command_error)?;
    discard_success_body(response).map_err(remote_command_error)
}

#[tauri::command]
pub fn remote_rename_job_speaker(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
    from_speaker: String,
    to_speaker: String,
) -> Result<MeetingJob, AppErrorDto> {
    require_main_window(window.label()).map_err(remote_command_error)?;
    let id = normalize_job_id(&id).map_err(remote_command_error)?;
    let from_speaker =
        normalize_speaker(&from_speaker, "原讲话人").map_err(remote_command_error)?;
    let to_speaker = normalize_speaker(&to_speaker, "新讲话人").map_err(remote_command_error)?;
    let context = remote_context(&app).map_err(remote_command_error)?;
    let capabilities = fetch_capabilities(&context).map_err(remote_command_error)?;
    require_operation(&capabilities, "transcript.speakers.rename").map_err(remote_command_error)?;
    request_job(
        &context,
        Method::POST,
        &["api", "jobs", id, "speakers", "rename"],
        Some(json!({
            "fromSpeaker": from_speaker,
            "toSpeaker": to_speaker,
        })),
        id,
    )
    .map_err(remote_command_error)
}

fn remote_command_error(_source: String) -> AppErrorDto {
    AppErrorDto::new("remote_service_unavailable", ErrorCategory::Network, true)
}

fn require_main_window(window_label: &str) -> LocalResult<()> {
    if window_label == "main" {
        Ok(())
    } else {
        Err("当前窗口无权访问远端会议服务。".into())
    }
}

fn remote_context(app: &AppHandle) -> LocalResult<RemoteContext> {
    let settings = local_db::get_settings(app)?;
    let api_token = normalize_api_token(&settings.api_token)?;
    let target =
        TrustedHttpTarget::resolve(&settings.backend_url, TrustedHttpPolicy::RemoteMeeting)
            .map_err(|error| format!("capability_unavailable: {error}"))?;
    let client = target
        .blocking_client(REQUEST_TIMEOUT)
        .map_err(|error| format!("capability_unavailable: {error}"))?;

    Ok(RemoteContext {
        client,
        target,
        api_token,
    })
}

fn normalize_api_token(value: &str) -> LocalResult<String> {
    let value = value.trim();
    if value.is_empty() {
        Err("capability_unavailable: 未配置远端 API Token，已拒绝发起网络请求。".into())
    } else {
        Ok(value.to_string())
    }
}

fn fetch_capabilities(context: &RemoteContext) -> LocalResult<RemoteMeetingCapabilities> {
    let response = send_request(context, Method::GET, &["api", "capabilities"], None)?;
    let capabilities: RemoteMeetingCapabilities = decode_json_response(response)?;
    validate_capabilities(capabilities)
}

fn validate_capabilities(
    mut capabilities: RemoteMeetingCapabilities,
) -> LocalResult<RemoteMeetingCapabilities> {
    if capabilities.protocol != PROTOCOL_NAME
        || capabilities.protocol_version != PROTOCOL_VERSION
        || capabilities.job_schema_version != JOB_SCHEMA_VERSION
    {
        return Err("capability_unavailable: 远端服务协议版本与当前客户端不兼容。".into());
    }
    capabilities.service_version = capabilities.service_version.trim().to_string();
    if capabilities.service_version.is_empty() {
        return Err("capability_unavailable: 远端 capabilities 缺少服务版本。".into());
    }

    let known = KNOWN_OPERATIONS.iter().copied().collect::<HashSet<_>>();
    let mut unique = HashSet::new();
    for operation in &capabilities.operations {
        if !known.contains(operation.as_str()) {
            return Err(format!(
                "capability_unavailable: 远端 capabilities 包含未知操作 {operation}。"
            ));
        }
        if !unique.insert(operation.as_str()) {
            return Err(format!(
                "capability_unavailable: 远端 capabilities 重复声明操作 {operation}。"
            ));
        }
    }

    if let Some(job_create) = capabilities.job_create.as_mut() {
        if !matches!(job_create.upload_mode.as_str(), "multipart" | "chunked")
            || job_create.max_files == 0
            || job_create.max_bytes_per_file == 0
            || job_create.extensions.is_empty()
            || job_create
                .extensions
                .iter()
                .any(|extension| extension.trim().is_empty())
        {
            return Err("capability_unavailable: 远端任务上传能力声明无效。".into());
        }
        job_create.extensions = job_create
            .extensions
            .iter()
            .map(|extension| extension.trim().to_ascii_lowercase())
            .collect();
    }
    if capabilities
        .operations
        .iter()
        .any(|item| item == "jobs.create")
        && capabilities.job_create.is_none()
    {
        return Err("capability_unavailable: 远端服务声明创建任务但未提供上传约束。".into());
    }

    if let Some(exports) = capabilities.exports.as_mut() {
        exports.formats = exports
            .formats
            .iter()
            .map(|format| format.trim().to_ascii_lowercase())
            .filter(|format| !format.is_empty())
            .collect();
    }
    Ok(capabilities)
}

fn require_operation(capabilities: &RemoteMeetingCapabilities, operation: &str) -> LocalResult<()> {
    if capabilities
        .operations
        .iter()
        .any(|candidate| candidate == operation)
    {
        Ok(())
    } else {
        Err(format!(
            "capability_unavailable: 远端服务未声明 {operation} 能力。"
        ))
    }
}

fn request_jobs(
    context: &RemoteContext,
    method: Method,
    path: &[&str],
    body: Option<Value>,
) -> LocalResult<Vec<MeetingJob>> {
    let response = send_request(context, method, path, body)?;
    let value: Value = decode_json_response(response)?;
    let values = value
        .as_array()
        .ok_or_else(|| "远端任务列表响应不是数组。".to_string())?;
    values.iter().cloned().map(parse_remote_job).collect()
}

fn request_job(
    context: &RemoteContext,
    method: Method,
    path: &[&str],
    body: Option<Value>,
    expected_job_id: &str,
) -> LocalResult<MeetingJob> {
    let response = send_request(context, method, path, body)?;
    let job = parse_remote_job(decode_json_response(response)?)?;
    require_matching_job_id(job, expected_job_id)
}

fn send_request(
    context: &RemoteContext,
    method: Method,
    path: &[&str],
    body: Option<Value>,
) -> LocalResult<Response> {
    let url = context
        .target
        .endpoint(path)
        .map_err(|error| error.to_string())?;
    let mut request = context
        .client
        .request(method, url)
        .header(ACCEPT, "application/json")
        .bearer_auth(&context.api_token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .map_err(|error| format!("远端服务请求失败: {error}"))?;
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(remote_response_error(response))
    }
}

fn decode_json_response<T: for<'de> Deserialize<'de>>(response: Response) -> LocalResult<T> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !content_type.starts_with("application/json") && !content_type.contains("+json") {
        return Err("远端服务返回了非 JSON 响应。".into());
    }
    let bytes = read_limited(response, MAX_RESPONSE_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|error| format!("解析远端 JSON 响应失败: {error}"))
}

fn discard_success_body(response: Response) -> LocalResult<()> {
    let _ = read_limited(response, MAX_ERROR_BYTES)?;
    Ok(())
}

fn remote_response_error(response: Response) -> String {
    let status = response.status();
    let body = read_limited(response, MAX_ERROR_BYTES).unwrap_or_default();
    let detail = serde_json::from_slice::<RemoteErrorEnvelope>(&body)
        .ok()
        .and_then(|envelope| envelope.error)
        .map(|error| {
            [error.code, error.message]
                .into_iter()
                .flatten()
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(": ")
        })
        .filter(|message| !message.is_empty());
    detail.unwrap_or_else(|| format!("远端服务返回 HTTP {status}"))
}

fn read_limited(response: Response, limit: u64) -> LocalResult<Vec<u8>> {
    let mut bytes = Vec::new();
    response
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取远端服务响应失败: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err("远端服务响应体超过客户端限制。".into());
    }
    Ok(bytes)
}

fn parse_remote_job(value: Value) -> LocalResult<MeetingJob> {
    let mut value = value;
    let object = value
        .as_object()
        .ok_or_else(|| "远端任务响应不是对象。".to_string())?;
    for field in ["id", "title", "createdAt", "overallStatus"] {
        if object.get(field).and_then(Value::as_str).is_none() {
            return Err(format!("远端任务响应缺少必需字段 {field}。"));
        }
    }
    for field in [
        "sourceFiles",
        "transcriptSegments",
        "speakerSegments",
        "summaryRuns",
    ] {
        if object.get(field).and_then(Value::as_array).is_none() {
            return Err(format!("远端任务响应字段 {field} 不是数组。"));
        }
    }
    if object.get("summary").and_then(Value::as_object).is_none() {
        return Err("远端任务响应字段 summary 不是对象。".into());
    }

    let explicit_diarization_status = object
        .get("diarizationStatus")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let normalized_status = match explicit_diarization_status.as_deref() {
        Some("completed") => DiarizationStatus::Completed,
        Some(value) => DiarizationStatus::try_from(value)
            .map_err(|error| format!("远端任务响应包含无效 diarizationStatus: {error}"))?,
        None => DiarizationStatus::LegacyUnverified,
    };
    let object = value
        .as_object_mut()
        .ok_or_else(|| "远端任务响应不是对象。".to_string())?;
    object.insert(
        "asrBackend".into(),
        object
            .get("asrBackend")
            .cloned()
            .unwrap_or_else(|| Value::String(AsrBackend::Unknown.as_str().into())),
    );
    object.insert(
        "diarizationStatus".into(),
        Value::String(normalized_status.as_str().into()),
    );
    object.insert(
        "warnings".into(),
        object.get("warnings").cloned().unwrap_or_else(|| {
            serde_json::to_value(vec![RunnerWarning {
                code: "remote_diarization_unverified".into(),
                message: "远端旧版任务未声明版本化说话人状态，默认仅使用逐字稿。".into(),
            }])
            .expect("serializable warning")
        }),
    );

    let mut job: MeetingJob = serde_json::from_value(value)
        .map_err(|error| format!("远端任务响应不符合 Job V1 schema: {error}"))?;
    if job.id.trim().is_empty() || job.title.trim().is_empty() {
        return Err("远端任务响应包含空的任务 ID 或标题。".into());
    }
    job.source = "remote".into();
    job.python_path = None;
    job.runner_script_path = None;
    Ok(job)
}

fn require_matching_job_id(job: MeetingJob, expected_job_id: &str) -> LocalResult<MeetingJob> {
    if job.id == expected_job_id {
        Ok(job)
    } else {
        Err("远端任务响应的 job.id 与请求 ID 不匹配。".into())
    }
}

fn normalize_job_id(value: &str) -> LocalResult<&str> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 {
        Err("任务 ID 无效。".into())
    } else {
        Ok(value)
    }
}

fn normalize_speaker<'a>(value: &'a str, label: &str) -> LocalResult<&'a str> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 {
        Err(format!("{label}名称无效。"))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_api_token, require_matching_job_id, validate_capabilities, MeetingJob,
        RemoteMeetingCapabilities,
    };

    #[test]
    fn capabilities_reject_unknown_operations() {
        let capabilities: RemoteMeetingCapabilities = serde_json::from_value(serde_json::json!({
            "protocol": "liberty-meeting",
            "protocolVersion": 1,
            "jobSchemaVersion": 1,
            "serviceVersion": "1.0.0",
            "operations": ["jobs.list", "jobs.unknown"]
        }))
        .expect("capabilities");

        assert!(validate_capabilities(capabilities).is_err());
    }

    #[test]
    fn api_token_is_required_and_trimmed_before_network_use() {
        assert!(normalize_api_token("").is_err());
        assert!(normalize_api_token(" \r\n ").is_err());
        assert_eq!(
            normalize_api_token("  secret-token  ").unwrap(),
            "secret-token"
        );
    }

    #[test]
    fn single_job_response_must_match_the_requested_id_exactly() {
        let matching = MeetingJob {
            id: "job-1".into(),
            ..MeetingJob::default()
        };
        assert!(require_matching_job_id(matching, "job-1").is_ok());

        for response_id in ["job-2", " job-1 "] {
            let mismatched = MeetingJob {
                id: response_id.into(),
                ..MeetingJob::default()
            };
            assert!(require_matching_job_id(mismatched, "job-1").is_err());
        }
    }
}
