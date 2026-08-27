use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::rand::{SecureRandom, SystemRandom};
use tauri::{AppHandle, Theme, WebviewWindow};

use crate::{
    infrastructure::runner_files,
    local_db::{self, LocalResult},
};

const AI_SUMMARY_WINDOW: &str = "ai-summary";
const MEETING_NOTES_WINDOW: &str = "meeting-notes";
const JOB_WORKBENCH_WINDOW: &str = "job-workbench";
const JOB_WINDOW_LABELS: [&str; 3] = [
    AI_SUMMARY_WINDOW,
    MEETING_NOTES_WINDOW,
    JOB_WORKBENCH_WINDOW,
];
const EDITOR_WINDOW_LABELS: [&str; 3] = ["model-editor", "template-editor", "member-editor"];
const THEMED_WINDOW_LABELS: [&str; 8] = [
    "main",
    AI_SUMMARY_WINDOW,
    MEETING_NOTES_WINDOW,
    JOB_WORKBENCH_WINDOW,
    "model-editor",
    "template-editor",
    "member-editor",
    "pet-store-item-detail",
];

static JOB_WINDOW_SCOPES: OnceLock<Mutex<JobWindowScopes>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
struct JobWindowScope {
    job_id: String,
    source: String,
    token: String,
}

#[derive(Debug, Default)]
struct JobWindowScopes {
    by_window: HashMap<String, JobWindowScope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeIssuance {
    Direct,
    FromParent(&'static str),
}

impl JobWindowScopes {
    fn replace(&mut self, window_label: &str, source: &str, job_id: &str, token: &str) {
        self.by_window.insert(
            window_label.to_string(),
            JobWindowScope {
                job_id: job_id.to_string(),
                source: source.to_string(),
                token: token.to_string(),
            },
        );
    }

    fn authorizes(
        &self,
        window_label: &str,
        source: &str,
        job_id: &str,
        token: Option<&str>,
    ) -> bool {
        let Some(token) = token.filter(|value| !value.is_empty() && value.trim() == *value) else {
            return false;
        };
        self.by_window.get(window_label).is_some_and(|scope| {
            scope.source == source && scope.job_id == job_id && scope.token == token
        })
    }

    fn replace_authorized_child(
        &mut self,
        parent_window_label: &str,
        child_window_label: &str,
        source: &str,
        job_id: &str,
        parent_token: Option<&str>,
        child_token: &str,
    ) -> LocalResult<()> {
        if !self.authorizes(parent_window_label, source, job_id, parent_token) {
            return Err("任务窗口作用域已失效，请从主窗口重新打开。".into());
        }
        self.replace(child_window_label, source, job_id, child_token);
        Ok(())
    }
}

#[tauri::command]
pub fn issue_job_window_scope(
    app: AppHandle,
    window: WebviewWindow,
    window_label: String,
    source: String,
    job_id: String,
    parent_scope_token: Option<String>,
) -> LocalResult<String> {
    let window_label = normalize_window_label(&window_label)?;
    let source = normalize_source(&source)?;
    let job_id = normalize_scoped_job_id(source, &job_id)?;
    let issuance = scope_issuance_policy(window.label(), window_label, source)?;

    if source == "local" {
        let job = local_db::get_job(&app, job_id)?;
        if job.source != "local" {
            return Err("任务来源与独立窗口作用域不匹配。".into());
        }
    }

    let token = generate_scope_token()?;
    let mut scopes = job_window_scopes()
        .lock()
        .map_err(|error| format!("任务窗口作用域锁异常: {error}"))?;
    match issuance {
        ScopeIssuance::Direct => scopes.replace(window_label, source, job_id, &token),
        ScopeIssuance::FromParent(parent_window_label) => scopes.replace_authorized_child(
            parent_window_label,
            window_label,
            source,
            job_id,
            parent_scope_token.as_deref(),
            &token,
        )?,
    }
    Ok(token)
}

#[tauri::command]
pub fn close_current_window(window: WebviewWindow) -> LocalResult<()> {
    require_window_label(window.label(), &JOB_WINDOW_LABELS, "关闭")?;
    window.close().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn destroy_current_window(window: WebviewWindow) -> LocalResult<()> {
    require_window_label(window.label(), &EDITOR_WINDOW_LABELS, "销毁")?;
    window.destroy().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_current_window_title(window: WebviewWindow, title: String) -> LocalResult<()> {
    require_window_label(window.label(), &EDITOR_WINDOW_LABELS, "修改标题")?;
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 160 {
        return Err("窗口标题格式无效。".into());
    }
    window.set_title(title).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_current_window_theme(window: WebviewWindow, theme: Option<String>) -> LocalResult<()> {
    require_window_label(window.label(), &THEMED_WINDOW_LABELS, "修改主题")?;
    let theme = match theme.as_deref() {
        None => None,
        Some("light") => Some(Theme::Light),
        Some("dark") => Some(Theme::Dark),
        _ => return Err("窗口主题格式无效。".into()),
    };
    window.set_theme(theme).map_err(|error| error.to_string())
}

pub(crate) fn authorize_job_window(
    window: &WebviewWindow,
    allowed_child_windows: &[&str],
    source: &str,
    job_id: &str,
    scope_token: Option<&str>,
) -> LocalResult<()> {
    if window.label() == "main" {
        return Ok(());
    }
    if !allowed_child_windows.contains(&window.label()) {
        return Err("当前窗口无权访问这个任务。".into());
    }
    let scopes = job_window_scopes()
        .lock()
        .map_err(|error| format!("任务窗口作用域锁异常: {error}"))?;
    if scopes.authorizes(window.label(), source, job_id, scope_token) {
        Ok(())
    } else {
        Err("任务窗口作用域已失效，请从主窗口重新打开。".into())
    }
}

pub(crate) fn ai_summary_window() -> &'static str {
    AI_SUMMARY_WINDOW
}

pub(crate) fn meeting_notes_window() -> &'static str {
    MEETING_NOTES_WINDOW
}

pub(crate) fn job_workbench_window() -> &'static str {
    JOB_WORKBENCH_WINDOW
}

fn job_window_scopes() -> &'static Mutex<JobWindowScopes> {
    JOB_WINDOW_SCOPES.get_or_init(|| Mutex::new(JobWindowScopes::default()))
}

fn scope_issuance_policy(
    requester_window_label: &str,
    target_window_label: &str,
    source: &str,
) -> LocalResult<ScopeIssuance> {
    match requester_window_label {
        "main" if source == "remote" && target_window_label != JOB_WORKBENCH_WINDOW => {
            Err("远端任务不支持这个独立窗口。".into())
        }
        "main" => Ok(ScopeIssuance::Direct),
        JOB_WORKBENCH_WINDOW
            if source == "local"
                && [AI_SUMMARY_WINDOW, MEETING_NOTES_WINDOW].contains(&target_window_label) =>
        {
            Ok(ScopeIssuance::FromParent(JOB_WORKBENCH_WINDOW))
        }
        JOB_WORKBENCH_WINDOW => Err("结果工作台无权创建这个任务窗口。".into()),
        _ => Err("当前窗口无权创建任务窗口作用域。".into()),
    }
}

fn normalize_window_label(value: &str) -> LocalResult<&str> {
    let value = value.trim();
    if JOB_WINDOW_LABELS.contains(&value) {
        Ok(value)
    } else {
        Err("不支持的任务窗口类型。".into())
    }
}

fn normalize_source(value: &str) -> LocalResult<&str> {
    let value = value.trim();
    match value {
        "local" | "remote" => Ok(value),
        _ => Err("任务来源无效。".into()),
    }
}

fn normalize_scoped_job_id<'a>(source: &str, job_id: &'a str) -> LocalResult<&'a str> {
    let job_id = job_id.trim();
    if source == "local" {
        runner_files::validate_job_id(job_id)?;
    } else if job_id.is_empty() || job_id.len() > 256 {
        return Err("远端任务 ID 无效。".into());
    }
    Ok(job_id)
}

fn require_window_label(actual: &str, allowed: &[&str], operation: &str) -> LocalResult<()> {
    if allowed.contains(&actual) {
        Ok(())
    } else {
        Err(format!("当前窗口无权{operation}窗口。"))
    }
}

fn generate_scope_token() -> LocalResult<String> {
    let mut bytes = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "无法生成任务窗口作用域令牌。".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::{
        generate_scope_token, normalize_source, normalize_window_label, require_window_label,
        scope_issuance_policy, JobWindowScopes, ScopeIssuance, AI_SUMMARY_WINDOW,
        EDITOR_WINDOW_LABELS, JOB_WINDOW_LABELS, JOB_WORKBENCH_WINDOW, MEETING_NOTES_WINDOW,
    };

    #[test]
    fn scope_is_bound_to_window_job_and_token() {
        let mut scopes = JobWindowScopes::default();
        scopes.replace("ai-summary", "local", "job-1700000000000-0", "token-a");

        assert!(scopes.authorizes(
            "ai-summary",
            "local",
            "job-1700000000000-0",
            Some("token-a")
        ));
        assert!(!scopes.authorizes(
            "ai-summary",
            "remote",
            "job-1700000000000-0",
            Some("token-a")
        ));
        assert!(!scopes.authorizes(
            "meeting-notes",
            "local",
            "job-1700000000000-0",
            Some("token-a")
        ));
        assert!(!scopes.authorizes(
            "ai-summary",
            "local",
            "job-1700000000000-1",
            Some("token-a")
        ));
        assert!(!scopes.authorizes(
            "ai-summary",
            "local",
            "job-1700000000000-0",
            Some("token-b")
        ));
        assert!(!scopes.authorizes("ai-summary", "local", "job-1700000000000-0", None));
    }

    #[test]
    fn replacing_scope_invalidates_previous_token() {
        let mut scopes = JobWindowScopes::default();
        scopes.replace("meeting-notes", "local", "job-1700000000000-0", "token-a");
        scopes.replace("meeting-notes", "local", "job-1700000000000-1", "token-b");

        assert!(!scopes.authorizes(
            "meeting-notes",
            "local",
            "job-1700000000000-0",
            Some("token-a")
        ));
        assert!(scopes.authorizes(
            "meeting-notes",
            "local",
            "job-1700000000000-1",
            Some("token-b")
        ));
    }

    #[test]
    fn scope_issuance_policy_enforces_window_topology() {
        for target in [
            AI_SUMMARY_WINDOW,
            MEETING_NOTES_WINDOW,
            JOB_WORKBENCH_WINDOW,
        ] {
            assert_eq!(
                scope_issuance_policy("main", target, "local"),
                Ok(ScopeIssuance::Direct)
            );
        }
        assert_eq!(
            scope_issuance_policy("main", JOB_WORKBENCH_WINDOW, "remote"),
            Ok(ScopeIssuance::Direct)
        );
        assert!(scope_issuance_policy("main", AI_SUMMARY_WINDOW, "remote").is_err());
        assert!(scope_issuance_policy("main", MEETING_NOTES_WINDOW, "remote").is_err());

        for target in [AI_SUMMARY_WINDOW, MEETING_NOTES_WINDOW] {
            assert_eq!(
                scope_issuance_policy(JOB_WORKBENCH_WINDOW, target, "local"),
                Ok(ScopeIssuance::FromParent(JOB_WORKBENCH_WINDOW))
            );
        }
        assert!(
            scope_issuance_policy(JOB_WORKBENCH_WINDOW, JOB_WORKBENCH_WINDOW, "local").is_err()
        );
        assert!(scope_issuance_policy(JOB_WORKBENCH_WINDOW, AI_SUMMARY_WINDOW, "remote").is_err());
        assert!(scope_issuance_policy(AI_SUMMARY_WINDOW, MEETING_NOTES_WINDOW, "local").is_err());
    }

    #[test]
    fn child_scope_requires_the_current_parent_token() {
        let mut scopes = JobWindowScopes::default();
        let job_id = "job-1700000000000-0";
        scopes.replace(JOB_WORKBENCH_WINDOW, "local", job_id, "parent-token-a");

        for parent_token in [
            None,
            Some(""),
            Some(" parent-token-a "),
            Some("wrong-token"),
        ] {
            assert!(scopes
                .replace_authorized_child(
                    JOB_WORKBENCH_WINDOW,
                    AI_SUMMARY_WINDOW,
                    "local",
                    job_id,
                    parent_token,
                    "child-token",
                )
                .is_err());
        }
        assert!(scopes
            .replace_authorized_child(
                JOB_WORKBENCH_WINDOW,
                AI_SUMMARY_WINDOW,
                "remote",
                job_id,
                Some("parent-token-a"),
                "child-token",
            )
            .is_err());
        assert!(scopes
            .replace_authorized_child(
                JOB_WORKBENCH_WINDOW,
                AI_SUMMARY_WINDOW,
                "local",
                "job-1700000000000-1",
                Some("parent-token-a"),
                "child-token",
            )
            .is_err());

        scopes.replace(JOB_WORKBENCH_WINDOW, "local", job_id, "parent-token-b");
        assert!(scopes
            .replace_authorized_child(
                JOB_WORKBENCH_WINDOW,
                AI_SUMMARY_WINDOW,
                "local",
                job_id,
                Some("parent-token-a"),
                "child-token-a",
            )
            .is_err());
        assert!(scopes
            .replace_authorized_child(
                JOB_WORKBENCH_WINDOW,
                AI_SUMMARY_WINDOW,
                "local",
                job_id,
                Some("parent-token-b"),
                "child-token-b",
            )
            .is_ok());
        assert!(scopes.authorizes(AI_SUMMARY_WINDOW, "local", job_id, Some("child-token-b")));
    }

    #[test]
    fn only_declared_job_windows_can_receive_scopes() {
        assert_eq!(normalize_window_label(" ai-summary "), Ok("ai-summary"));
        assert_eq!(normalize_window_label("meeting-notes"), Ok("meeting-notes"));
        assert_eq!(normalize_window_label("job-workbench"), Ok("job-workbench"));
        assert!(normalize_window_label("model-editor").is_err());
        assert_eq!(normalize_source(" local "), Ok("local"));
        assert!(normalize_source("web").is_err());
    }

    #[test]
    fn generated_tokens_are_url_safe_and_unique() {
        let first = generate_scope_token().expect("first token");
        let second = generate_scope_token().expect("second token");
        assert_eq!(first.len(), 43);
        assert!(first
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'));
        assert_ne!(first, second);
    }

    #[test]
    fn current_window_operations_reject_other_window_roles() {
        assert!(require_window_label("ai-summary", &JOB_WINDOW_LABELS, "关闭").is_ok());
        assert!(require_window_label("job-workbench", &JOB_WINDOW_LABELS, "关闭").is_ok());
        assert!(require_window_label("main", &JOB_WINDOW_LABELS, "关闭").is_err());
        assert!(require_window_label("model-editor", &EDITOR_WINDOW_LABELS, "销毁").is_ok());
        assert!(require_window_label("main", &EDITOR_WINDOW_LABELS, "销毁").is_err());
    }
}
