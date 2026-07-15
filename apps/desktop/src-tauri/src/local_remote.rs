use std::{
    collections::HashSet,
    io::Read,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    time::Duration,
};

use reqwest::{
    blocking::{Client, Response},
    header::{ACCEPT, CONTENT_TYPE},
    redirect::Policy,
    Method, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, WebviewWindow};

use crate::local_db::{self, LocalResult, MeetingJob};

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
    base_url: Url,
    api_token: String,
}

struct ResolvedRemoteTarget {
    base_url: Url,
    dns_override: Option<(String, Vec<SocketAddr>)>,
}

#[tauri::command]
pub fn get_remote_capabilities(
    app: AppHandle,
    window: WebviewWindow,
) -> LocalResult<RemoteMeetingCapabilities> {
    require_main_window(window.label())?;
    let context = remote_context(&app)?;
    fetch_capabilities(&context)
}

#[tauri::command]
pub fn remote_list_jobs(app: AppHandle, window: WebviewWindow) -> LocalResult<Vec<MeetingJob>> {
    require_main_window(window.label())?;
    let context = remote_context(&app)?;
    let capabilities = fetch_capabilities(&context)?;
    require_operation(&capabilities, "jobs.list")?;
    request_jobs(&context, Method::GET, &["api", "jobs"], None)
}

#[tauri::command]
pub fn remote_get_job(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> LocalResult<MeetingJob> {
    require_main_window(window.label())?;
    let id = normalize_job_id(&id)?;
    let context = remote_context(&app)?;
    let capabilities = fetch_capabilities(&context)?;
    require_operation(&capabilities, "jobs.read")?;
    request_job(&context, Method::GET, &["api", "jobs", id], None, id)
}

#[tauri::command]
pub fn remote_get_job_result(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> LocalResult<MeetingJob> {
    require_main_window(window.label())?;
    let id = normalize_job_id(&id)?;
    let context = remote_context(&app)?;
    let capabilities = fetch_capabilities(&context)?;
    require_operation(&capabilities, "jobs.result.read")?;
    request_job(
        &context,
        Method::GET,
        &["api", "jobs", id, "result"],
        None,
        id,
    )
}

#[tauri::command]
pub fn remote_retry_job(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> LocalResult<MeetingJob> {
    require_main_window(window.label())?;
    let id = normalize_job_id(&id)?;
    let context = remote_context(&app)?;
    let capabilities = fetch_capabilities(&context)?;
    require_operation(&capabilities, "jobs.retry")?;
    request_job(
        &context,
        Method::POST,
        &["api", "jobs", id, "retry"],
        None,
        id,
    )
}

#[tauri::command]
pub fn remote_delete_job(app: AppHandle, window: WebviewWindow, id: String) -> LocalResult<()> {
    require_main_window(window.label())?;
    let id = normalize_job_id(&id)?;
    let context = remote_context(&app)?;
    let capabilities = fetch_capabilities(&context)?;
    require_operation(&capabilities, "jobs.delete")?;
    let response = send_request(&context, Method::DELETE, &["api", "jobs", id], None)?;
    discard_success_body(response)
}

#[tauri::command]
pub fn remote_rename_job_speaker(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
    from_speaker: String,
    to_speaker: String,
) -> LocalResult<MeetingJob> {
    require_main_window(window.label())?;
    let id = normalize_job_id(&id)?;
    let from_speaker = normalize_speaker(&from_speaker, "原讲话人")?;
    let to_speaker = normalize_speaker(&to_speaker, "新讲话人")?;
    let context = remote_context(&app)?;
    let capabilities = fetch_capabilities(&context)?;
    require_operation(&capabilities, "transcript.speakers.rename")?;
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
    let target = resolve_remote_target(&settings.backend_url)?;
    let mut client_builder = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(Policy::none())
        .no_proxy()
        .https_only(target.base_url.scheme() == "https");
    if let Some((domain, addresses)) = target.dns_override.as_ref() {
        client_builder = client_builder.resolve_to_addrs(domain, addresses);
    }
    let client = client_builder
        .build()
        .map_err(|error| format!("初始化远端服务客户端失败: {error}"))?;

    Ok(RemoteContext {
        client,
        base_url: target.base_url,
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

fn resolve_remote_target(value: &str) -> LocalResult<ResolvedRemoteTarget> {
    let base_url = parse_remote_base_url(value)?;
    let host = base_url
        .host_str()
        .ok_or_else(|| "capability_unavailable: 远端服务地址缺少主机名。".to_string())?
        .to_string();

    if literal_host_ip(&base_url).is_some() {
        validate_remote_destination(&base_url, &[])?;
        return Ok(ResolvedRemoteTarget {
            base_url,
            dns_override: None,
        });
    }

    if base_url.scheme() != "https" {
        return Err(
            "capability_unavailable: 携带 API Token 的 HTTP 请求仅允许 IP 字面量 loopback 本机端点。"
                .into(),
        );
    }

    let port = base_url
        .port_or_known_default()
        .ok_or_else(|| "capability_unavailable: 远端服务地址缺少有效端口。".to_string())?;
    let addresses = resolve_domain_addresses(&host, port)?;
    let resolved_ips = addresses
        .iter()
        .map(|address| address.ip())
        .collect::<Vec<_>>();
    validate_remote_destination(&base_url, &resolved_ips)?;

    Ok(ResolvedRemoteTarget {
        base_url,
        dns_override: Some((host, addresses)),
    })
}

fn parse_remote_base_url(value: &str) -> LocalResult<Url> {
    let mut base_url = Url::parse(value.trim())
        .map_err(|_| "capability_unavailable: 远端服务地址不是有效 URL。".to_string())?;
    if !matches!(base_url.scheme(), "http" | "https") {
        return Err("capability_unavailable: 远端服务只支持 HTTP(S) URL。".into());
    }
    if !base_url.username().is_empty() || base_url.password().is_some() {
        return Err("capability_unavailable: 远端服务地址不能内嵌用户名或密码。".into());
    }
    if base_url.host_str().is_none() {
        return Err("capability_unavailable: 远端服务地址缺少主机名。".into());
    }
    base_url.set_query(None);
    base_url.set_fragment(None);
    Ok(base_url)
}

fn resolve_domain_addresses(host: &str, port: u16) -> LocalResult<Vec<SocketAddr>> {
    let resolved = (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("capability_unavailable: 解析远端服务域名失败: {error}"))?;
    let mut seen = HashSet::new();
    let addresses = resolved
        .filter(|address| seen.insert(*address))
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        Err("capability_unavailable: 远端服务域名未解析到任何地址。".into())
    } else {
        Ok(addresses)
    }
}

fn validate_remote_destination(base_url: &Url, resolved_ips: &[IpAddr]) -> LocalResult<()> {
    if base_url.host_str().is_none() {
        return Err("capability_unavailable: 远端服务地址缺少主机名。".into());
    }

    if let Some(literal_ip) = literal_host_ip(base_url) {
        if is_loopback_ip(literal_ip) {
            return Ok(());
        }
        if base_url.scheme() != "https" {
            return Err(
                "capability_unavailable: 禁止通过非 loopback HTTP 端点传输 API Token。".into(),
            );
        }
        if !is_public_remote_ip(literal_ip) {
            return Err("capability_unavailable: 远端服务地址指向受限网络。".into());
        }
        return Ok(());
    }

    if base_url.scheme() != "https" {
        return Err(
            "capability_unavailable: 携带 API Token 的 HTTP 请求仅允许 IP 字面量 loopback 本机端点。"
                .into(),
        );
    }
    if resolved_ips.is_empty() {
        return Err("capability_unavailable: 远端服务域名缺少已验证的解析地址。".into());
    }
    if resolved_ips.iter().any(|ip| !is_public_remote_ip(*ip)) {
        return Err(
            "capability_unavailable: 远端服务域名解析到私网、loopback、link-local、metadata 或保留地址。"
                .into(),
        );
    }
    Ok(())
}

fn literal_host_ip(url: &Url) -> Option<IpAddr> {
    let host = url.host_str()?;
    host.strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host)
        .parse()
        .ok()
}

fn is_loopback_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || mapped_ipv4(ip)
                    .map(|mapped| mapped.is_loopback())
                    .unwrap_or(false)
        }
    }
}

fn is_public_remote_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => mapped_ipv4(ip)
            .map(is_public_ipv4)
            .unwrap_or_else(|| is_public_ipv6(ip)),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [first, second, third, fourth] = ip.octets();
    if first == 0
        || first == 10
        || first == 127
        || first >= 224
        || (first == 100 && (64..=127).contains(&second))
        || (first == 169 && second == 254)
        || (first == 172 && (16..=31).contains(&second))
        || (first == 192 && second == 168)
        || (first == 192 && second == 0 && third == 0)
        || (first == 192 && second == 0 && third == 2)
        || (first == 192 && second == 88 && third == 99)
        || (first == 198 && matches!(second, 18 | 19))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113)
    {
        return false;
    }

    // Azure exposes host metadata on this otherwise globally-routable virtual address.
    [first, second, third, fourth] != [168, 63, 129, 16]
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    if segments[0] & 0xe000 != 0x2000 {
        return false;
    }

    let special_2001 = segments[0] == 0x2001
        && (segments[1] == 0
            || (segments[1] == 2 && segments[2] == 0)
            || (segments[1] & 0xfff0) == 0x0010
            || (segments[1] & 0xfff0) == 0x0020
            || segments[1] == 0x0db8);
    let documentation_3fff = segments[0] == 0x3fff && (segments[1] & 0xf000) == 0;
    !special_2001 && segments[0] != 0x2002 && !documentation_3fff
}

fn mapped_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ip.segments();
    if segments[0] == 0
        && segments[1] == 0
        && segments[2] == 0
        && segments[3] == 0
        && segments[4] == 0
        && segments[5] == 0xffff
    {
        Some(Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ))
    } else {
        None
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
    let url = endpoint(&context.base_url, path)?;
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

fn endpoint(base_url: &Url, path: &[&str]) -> LocalResult<Url> {
    let mut url = base_url.clone();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "远端服务地址不能作为 API 基础地址。".to_string())?;
        segments.pop_if_empty();
        for segment in path {
            segments.push(segment);
        }
    }
    Ok(url)
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
    use std::net::IpAddr;

    use super::{
        endpoint, normalize_api_token, parse_remote_base_url, require_matching_job_id,
        validate_capabilities, validate_remote_destination, MeetingJob, RemoteMeetingCapabilities,
    };
    use reqwest::Url;

    #[test]
    fn endpoint_preserves_configured_base_path_and_encodes_ids() {
        let base = Url::parse("https://example.com/liberty/").expect("base URL");
        let url = endpoint(&base, &["api", "jobs", "job / one"]).expect("endpoint");
        assert_eq!(
            url.as_str(),
            "https://example.com/liberty/api/jobs/job%20%2F%20one"
        );
    }

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
    fn http_exception_requires_a_literal_loopback_address() {
        for value in ["http://127.0.0.1:8787", "http://[::1]:8787"] {
            let url = Url::parse(value).expect("loopback URL");
            assert!(validate_remote_destination(&url, &[]).is_ok(), "{value}");
        }

        let localhost = Url::parse("http://localhost:8787").expect("localhost URL");
        let loopback = ["127.0.0.1".parse::<IpAddr>().unwrap()];
        assert!(validate_remote_destination(&localhost, &loopback).is_err());

        let public_http = Url::parse("http://8.8.8.8").expect("public HTTP URL");
        assert!(validate_remote_destination(&public_http, &[]).is_err());
    }

    #[test]
    fn destination_rejects_private_link_local_metadata_and_mixed_dns_answers() {
        let base = Url::parse("https://api.example.com").expect("remote URL");
        let public = [
            "8.8.8.8".parse::<IpAddr>().unwrap(),
            "2606:4700:4700::1111".parse::<IpAddr>().unwrap(),
        ];
        assert!(validate_remote_destination(&base, &public).is_ok());

        for blocked in [
            "10.0.0.1",
            "100.100.100.200",
            "127.0.0.1",
            "169.254.169.254",
            "168.63.129.16",
            "192.168.1.1",
            "224.0.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "2001:db8::1",
            "::ffff:10.0.0.1",
            "::ffff:172.31.9.7",
        ] {
            let addresses = [blocked.parse::<IpAddr>().unwrap()];
            assert!(
                validate_remote_destination(&base, &addresses).is_err(),
                "accepted blocked address {blocked}"
            );
        }

        let mixed = [
            "8.8.8.8".parse::<IpAddr>().unwrap(),
            "10.0.0.1".parse::<IpAddr>().unwrap(),
        ];
        assert!(validate_remote_destination(&base, &mixed).is_err());

        let private_literal = Url::parse("https://192.168.1.1").expect("private URL");
        assert!(validate_remote_destination(&private_literal, &[]).is_err());
    }

    #[test]
    fn base_url_normalization_preserves_path_and_removes_query_and_fragment() {
        let url = parse_remote_base_url(" https://example.com/liberty/?debug=1#fragment ")
            .expect("base URL");
        assert_eq!(url.as_str(), "https://example.com/liberty/");
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
