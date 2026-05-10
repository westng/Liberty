use crate::process_utils::configure_background_process;
use serde::Serialize;
use std::{
    process::Command,
    sync::{Mutex, OnceLock},
};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

type LocalResult<T> = Result<T, String>;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMetrics {
    pub cpu_percent: f32,
    pub memory_mb: u64,
}

struct ProcessMetricsSampler {
    system: System,
    pid: Pid,
}

static PROCESS_METRICS_SAMPLER: OnceLock<Mutex<ProcessMetricsSampler>> = OnceLock::new();

#[tauri::command]
pub fn open_external_url(url: String) -> LocalResult<()> {
    let normalized = url.trim();
    if normalized.is_empty() {
        return Err("URL 不能为空。".into());
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("open");
        cmd.arg(normalized);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", normalized]);
        configure_background_process(&mut cmd);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(normalized);
        cmd
    };

    configure_background_process(&mut command)
        .spawn()
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_process_metrics() -> LocalResult<ProcessMetrics> {
    let sampler = PROCESS_METRICS_SAMPLER.get_or_init(|| {
        let pid = Pid::from_u32(std::process::id());
        let mut system = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new().with_cpu().with_memory()),
        );
        system.refresh_process_specifics(pid, ProcessRefreshKind::new().with_cpu().with_memory());
        Mutex::new(ProcessMetricsSampler { system, pid })
    });

    let mut sampler = sampler.lock().map_err(|err| err.to_string())?;
    let pid = sampler.pid;
    sampler
        .system
        .refresh_process_specifics(pid, ProcessRefreshKind::new().with_cpu().with_memory());

    let process = sampler
        .system
        .process(pid)
        .ok_or_else(|| "无法读取当前进程指标。".to_string())?;

    Ok(ProcessMetrics {
        cpu_percent: (process.cpu_usage() * 10.0).round() / 10.0,
        memory_mb: ((process.memory() as f64) / (1024.0 * 1024.0)).round() as u64,
    })
}
