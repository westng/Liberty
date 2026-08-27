use crate::process_utils::configure_background_process;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    process::Command,
    sync::{Mutex, OnceLock},
};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

#[cfg(target_os = "windows")]
use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt, ptr};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

type LocalResult<T> = Result<T, String>;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMetrics {
    pub cpu_percent: f32,
    pub memory_mb: u64,
}

struct ProcessMetricsSampler {
    system: System,
    root_pid: Pid,
}

static PROCESS_METRICS_SAMPLER: OnceLock<Mutex<ProcessMetricsSampler>> = OnceLock::new();

#[tauri::command]
pub fn open_external_url(url: String) -> LocalResult<()> {
    let normalized = validate_external_url(&url)?;

    #[cfg(target_os = "windows")]
    {
        let wide_url = OsStr::new(&normalized)
            .encode_wide()
            .chain(once(0))
            .collect::<Vec<_>>();
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                ptr::null(),
                wide_url.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize <= 32 {
            return Err(format!(
                "Windows 无法打开外部链接，错误码：{}",
                result as isize
            ));
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut cmd = Command::new("open");
            cmd.arg(&normalized);
            cmd
        };

        #[cfg(all(unix, not(target_os = "macos")))]
        let mut command = {
            let mut cmd = Command::new("xdg-open");
            cmd.arg(&normalized);
            cmd
        };

        configure_background_process(&mut command)
            .spawn()
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn validate_external_url(value: &str) -> LocalResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err("URL 不能为空。".into());
    }
    if normalized
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("URL 不能包含空白或控制字符。".into());
    }

    let parsed = reqwest::Url::parse(normalized).map_err(|err| format!("URL 无效：{err}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("仅允许打开 HTTP 或 HTTPS 外部链接。".into());
    }
    Ok(parsed.into())
}

#[tauri::command]
pub fn prompt_pet_name(
    title: String,
    message: String,
    default_value: String,
) -> LocalResult<Option<String>> {
    let title = normalize_dialog_text(&title);
    let message = normalize_dialog_text(&message);
    let default_value = normalize_dialog_text(&default_value);

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"set dialogResult to display dialog "{}" default answer "{}" with title "{}" buttons {{"取消", "保存"}} default button "保存" cancel button "取消"
return text returned of dialogResult"#,
            escape_applescript(&message),
            escape_applescript(&default_value),
            escape_applescript(&title)
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|err| err.to_string())?;

        if output.status.success() {
            return Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ));
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("-128") || stderr.to_lowercase().contains("user canceled") {
            return Ok(None);
        }

        Err(stderr.trim().to_string())
    }

    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.Interaction]::InputBox('{message}', '{title}', '{default_value}')",
            message = escape_powershell_single_quoted(&message),
            title = escape_powershell_single_quoted(&title),
            default_value = escape_powershell_single_quoted(&default_value),
        );
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-STA", "-Command", &script]);
        configure_background_process(&mut command);
        let output = command.output().map_err(|err| err.to_string())?;

        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout)
                .trim_end_matches(&['\r', '\n'][..])
                .to_string();
            if value.is_empty() {
                return Ok(None);
            }
            return Ok(Some(value));
        }

        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = (title, message, default_value);
        Err("当前平台暂不支持系统级文本输入弹窗。".into())
    }
}

#[tauri::command]
pub fn get_process_metrics() -> LocalResult<ProcessMetrics> {
    let sampler = PROCESS_METRICS_SAMPLER.get_or_init(|| {
        let root_pid = Pid::from_u32(std::process::id());
        let mut system = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new().with_cpu().with_memory()),
        );
        system.refresh_processes_specifics(ProcessRefreshKind::new().with_cpu().with_memory());
        Mutex::new(ProcessMetricsSampler { system, root_pid })
    });

    let mut sampler = sampler.lock().map_err(|err| err.to_string())?;
    sampler
        .system
        .refresh_processes_specifics(ProcessRefreshKind::new().with_cpu().with_memory());
    let descendants = process_tree_members(&sampler.system, sampler.root_pid);
    if !descendants.contains(&sampler.root_pid) {
        return Err("无法读取当前进程指标。".to_string());
    }
    let (cpu_percent, memory_bytes) = descendants
        .iter()
        .filter_map(|pid| sampler.system.process(*pid))
        .fold((0.0_f32, 0_u64), |(cpu, memory), process| {
            (
                cpu + process.cpu_usage(),
                memory.saturating_add(process.memory()),
            )
        });

    Ok(ProcessMetrics {
        cpu_percent: (cpu_percent * 10.0).round() / 10.0,
        memory_mb: ((memory_bytes as f64) / (1024.0 * 1024.0)).round() as u64,
    })
}

fn process_tree_members(system: &System, root_pid: Pid) -> HashSet<Pid> {
    let parents = system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process.parent()))
        .collect::<HashMap<_, _>>();
    process_tree_members_from_parents(root_pid, &parents)
}

fn process_tree_members_from_parents(
    root_pid: Pid,
    parents: &HashMap<Pid, Option<Pid>>,
) -> HashSet<Pid> {
    if !parents.contains_key(&root_pid) {
        return HashSet::new();
    }
    let mut members = HashSet::from([root_pid]);
    loop {
        let previous_len = members.len();
        for (pid, parent) in parents {
            if parent.is_some_and(|parent| members.contains(&parent)) {
                members.insert(*pid);
            }
        }
        if members.len() == previous_len {
            return members;
        }
    }
}

fn normalize_dialog_text(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_string()
}

#[cfg(target_os = "macos")]
fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::{process_tree_members_from_parents, validate_external_url};
    use std::collections::{HashMap, HashSet};
    use sysinfo::Pid;

    #[test]
    fn external_urls_are_limited_to_http_and_https() {
        assert!(validate_external_url("https://example.com/path?q=1").is_ok());
        assert!(validate_external_url("file:///tmp/payload").is_err());
        assert!(validate_external_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn external_urls_reject_shell_control_whitespace() {
        assert!(validate_external_url("https://example.com & calc.exe").is_err());
        assert!(validate_external_url("https://example.com\n--flag").is_err());
    }

    #[test]
    fn process_tree_includes_all_descendant_generations() {
        let root = Pid::from_u32(10);
        let child = Pid::from_u32(11);
        let grandchild = Pid::from_u32(12);
        let unrelated = Pid::from_u32(20);
        let parents = HashMap::from([
            (root, None),
            (child, Some(root)),
            (grandchild, Some(child)),
            (unrelated, None),
        ]);

        assert_eq!(
            process_tree_members_from_parents(root, &parents),
            HashSet::from([root, child, grandchild])
        );
    }

    #[test]
    fn process_tree_is_empty_when_root_is_missing() {
        let root = Pid::from_u32(10);
        let child = Pid::from_u32(11);
        let parents = HashMap::from([(child, Some(root))]);

        assert!(process_tree_members_from_parents(root, &parents).is_empty());
    }
}
