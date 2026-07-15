use std::{
    io::Read,
    path::Path,
    process::{Child, Command, ExitStatus},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, System};
use tauri::AppHandle;

use crate::{
    infrastructure::process_logs,
    local_db::{AppSettings, LocalResult, MeetingJob},
    local_runtime::ResolvedPythonRuntime,
    process_utils::configure_background_process,
};

const MAX_LOCAL_ASR_THREADS: u32 = 32;
const PROCESS_IDENTITY_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerProcessIdentity {
    version: u8,
    start_time: u64,
    executable: String,
    command: Vec<String>,
}

pub struct RunnerCommandInput<'a> {
    pub runtime: &'a ResolvedPythonRuntime,
    pub settings: &'a AppSettings,
    pub job: &'a MeetingJob,
    pub job_dir: &'a Path,
    pub runner_script_path: &'a Path,
    pub input_file: &'a Path,
    pub runtime_threads: u32,
}

#[derive(Debug)]
pub enum RunnerExit {
    Finished(ExitStatus),
    Cancelled,
}

pub fn validate_runtime_tools(
    job_dir: &Path,
    input_file: &str,
    ffmpeg_path: Option<&str>,
) -> LocalResult<()> {
    let input_suffix = Path::new(input_file)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    if input_suffix == "wav" {
        return Ok(());
    }

    let ffmpeg = ffmpeg_path
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "当前文件需要 ffmpeg 进行音频解码，但本地运行环境中未找到 ffmpeg。".to_string()
        })?;

    process_logs::append_line(job_dir, &format!("[runner] validating ffmpeg={ffmpeg}"))?;
    let mut command = Command::new(ffmpeg);
    command.arg("-hide_banner").arg("-version");
    configure_background_process(&mut command);
    let output = command.output().map_err(|err| {
        format!(
            "任务启动前检测 ffmpeg 失败：{}。请重新安装本地运行环境。",
            err
        )
    })?;

    if output.status.success() {
        return Ok(());
    }

    if !output.stdout.is_empty() {
        process_logs::append_line(job_dir, &String::from_utf8_lossy(&output.stdout))?;
    }
    if !output.stderr.is_empty() {
        process_logs::append_line(job_dir, &String::from_utf8_lossy(&output.stderr))?;
    }

    Err(format!(
        "任务启动前检测 ffmpeg 失败，退出码 {}。请重新安装本地运行环境。",
        output.status.code().unwrap_or(-1)
    ))
}

pub fn stream_child_logs<F, C, H>(
    app: &AppHandle,
    job_id: &str,
    job_dir: &Path,
    child: &mut Child,
    mut sync_log: F,
    mut should_cancel: C,
    mut heartbeat: H,
) -> LocalResult<RunnerExit>
where
    F: FnMut(&AppHandle, &str, &Path) -> LocalResult<()>,
    C: FnMut() -> bool,
    H: FnMut(u32) -> LocalResult<()>,
{
    let result = stream_child_logs_inner(
        app,
        job_id,
        job_dir,
        child,
        &mut sync_log,
        &mut should_cancel,
        &mut heartbeat,
    );
    match result {
        Ok(exit) => Ok(exit),
        Err(error) => match terminate_runner(child) {
            Ok(()) => Err(error),
            Err(termination_error) => Err(format!("{error}; {termination_error}")),
        },
    }
}

fn stream_child_logs_inner<F, C, H>(
    app: &AppHandle,
    job_id: &str,
    job_dir: &Path,
    child: &mut Child,
    sync_log: &mut F,
    should_cancel: &mut C,
    heartbeat: &mut H,
) -> LocalResult<RunnerExit>
where
    F: FnMut(&AppHandle, &str, &Path) -> LocalResult<()>,
    C: FnMut() -> bool,
    H: FnMut(u32) -> LocalResult<()>,
{
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "本地 Python 处理进程未返回 stdout 管道。".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "本地 Python 处理进程未返回 stderr 管道。".to_string())?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    spawn_log_reader(stdout, tx.clone());
    spawn_log_reader(stderr, tx.clone());
    drop(tx);

    let mut last_sync_at = Instant::now();
    let mut last_heartbeat_at = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => process_logs::append_bytes(job_dir, &chunk)?,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {}
        }
        for chunk in rx.try_iter() {
            process_logs::append_bytes(job_dir, &chunk)?;
        }

        if should_cancel() {
            terminate_runner(child)?;
            sync_log(app, job_id, job_dir)?;
            return Ok(RunnerExit::Cancelled);
        }

        if last_heartbeat_at.elapsed() >= Duration::from_secs(1) {
            heartbeat(child.id())?;
            last_heartbeat_at = Instant::now();
        }
        if last_sync_at.elapsed() >= Duration::from_secs(1) {
            sync_log(app, job_id, job_dir)?;
            last_sync_at = Instant::now();
        }
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            for chunk in rx.try_iter() {
                process_logs::append_bytes(job_dir, &chunk)?;
            }
            sync_log(app, job_id, job_dir)?;
            return Ok(RunnerExit::Finished(status));
        }
    }
}

pub fn capture_runner_process_identity(pid: u32) -> LocalResult<String> {
    for _ in 0..10 {
        if let Some(identity) = read_process_identity(pid) {
            if !identity.executable.is_empty() || !identity.command.is_empty() {
                return serde_json::to_string(&identity)
                    .map_err(|err| format!("无法序列化 Runner 进程身份: {err}"));
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!("无法读取 Runner 进程 {pid} 的启动身份。"))
}

pub fn terminate_persisted_runner(
    pid: Option<u32>,
    process_identity: Option<&str>,
) -> LocalResult<()> {
    let Some(pid) = pid else {
        return Ok(());
    };
    let process_identity = process_identity
        .filter(|identity| !identity.trim().is_empty())
        .ok_or_else(|| format!("Runner 进程 {pid} 缺少可核验的启动身份。"))?;
    let Some(current) = read_process_identity(pid) else {
        return Ok(());
    };
    if !process_identity_matches(process_identity, &current)? {
        return Ok(());
    }

    terminate_process_tree_by_pid(pid)?;
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        let still_matches = read_process_identity(pid)
            .map(|identity| process_identity_matches(process_identity, &identity))
            .transpose()?
            .unwrap_or(false);
        if !still_matches {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!("终止旧 Runner 进程树超时: PID {pid}"))
}

fn read_process_identity(pid: u32) -> Option<RunnerProcessIdentity> {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    if !system.refresh_process_specifics(pid, ProcessRefreshKind::everything()) {
        return None;
    }
    let process = system.process(pid)?;
    Some(RunnerProcessIdentity {
        version: PROCESS_IDENTITY_VERSION,
        start_time: process.start_time(),
        executable: process
            .exe()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        command: process.cmd().to_vec(),
    })
}

fn process_identity_matches(persisted: &str, current: &RunnerProcessIdentity) -> LocalResult<bool> {
    match serde_json::from_str::<RunnerProcessIdentity>(persisted) {
        Ok(expected) => {
            if expected.version != PROCESS_IDENTITY_VERSION {
                return Err(format!(
                    "不支持的 Runner 进程身份版本: {}",
                    expected.version
                ));
            }
            Ok(&expected == current)
        }
        Err(json_error) => legacy_process_identity_matches(persisted, current)
            .ok_or_else(|| format!("Runner 进程身份无效，无法安全恢复: {json_error}")),
    }
}

fn legacy_process_identity_matches(
    persisted: &str,
    current: &RunnerProcessIdentity,
) -> Option<bool> {
    let parts = persisted.split('|').collect::<Vec<_>>();
    if parts.len() != 5 {
        return None;
    }
    let executable = parts[0];
    let runner_script = parts[1];
    let job_id = parts[2];
    let attempt_id = parts[3].parse::<u64>().ok()?;
    let lease_token = parts[4].parse::<u64>().ok()?;
    let executable_matches = current.executable == executable
        || current
            .command
            .first()
            .is_some_and(|argument| argument == executable);
    let script_matches = current
        .command
        .iter()
        .any(|argument| argument == runner_script);
    let attempt_marker = format!("attempt-{attempt_id}-{lease_token}");
    let job_dir_matches = current.command.windows(2).any(|arguments| {
        arguments[0] == "--job-dir"
            && arguments[1].contains(job_id)
            && arguments[1].contains(&attempt_marker)
    });
    Some(executable_matches && script_matches && job_dir_matches)
}

pub fn normalize_local_asr_device(settings: &AppSettings) -> String {
    match settings.local_asr_device.as_str() {
        "cpu" => "cpu".into(),
        "mps" => "mps".into(),
        "cuda" => "cuda".into(),
        _ => "auto".into(),
    }
}

pub fn resolve_local_asr_threads(settings: &AppSettings) -> u32 {
    if settings.local_asr_threads > 0 {
        return settings.local_asr_threads.clamp(1, MAX_LOCAL_ASR_THREADS);
    }

    let available = std::thread::available_parallelism()
        .map(|value| value.get() as u32)
        .unwrap_or(4);

    available.clamp(1, MAX_LOCAL_ASR_THREADS)
}

pub fn build_runner_command(input: RunnerCommandInput<'_>) -> Command {
    let mut command = Command::new(&input.runtime.python_path);
    command
        .env("OMP_NUM_THREADS", input.runtime_threads.to_string())
        .env("MKL_NUM_THREADS", input.runtime_threads.to_string())
        .env("NUMEXPR_NUM_THREADS", input.runtime_threads.to_string())
        .env("KMP_DUPLICATE_LIB_OK", "TRUE")
        .env("LIBERTY_ASR_BACKEND", &input.runtime.asr_backend)
        .env("FUNASR_DEVICE", normalize_local_asr_device(input.settings))
        .env(
            "FUNASR_BATCH_SIZE_S",
            input.settings.local_asr_batch_size_seconds.to_string(),
        )
        .arg(input.runner_script_path)
        .arg("--job-dir")
        .arg(input.job_dir)
        .arg("--input")
        .arg(input.input_file)
        .arg("--lang")
        .arg(&input.job.lang)
        .arg("--speaker")
        .arg(if input.job.enable_speaker {
            "true"
        } else {
            "false"
        })
        .arg("--hotwords")
        .arg(input.job.hotwords.join(","));

    if let Some(ffmpeg_path) = input.runtime.ffmpeg_path.as_deref() {
        command.env("LIBERTY_FFMPEG_PATH", ffmpeg_path);
    }

    if let Some(models_root) = input.runtime.models_root.as_deref() {
        command
            .env(
                "MODELSCOPE_CACHE",
                Path::new(models_root).join("modelscope"),
            )
            .env("HF_HOME", Path::new(models_root).join("huggingface"))
            .env("TORCH_HOME", Path::new(models_root).join("torch"));
    }

    command
}

pub fn configure_runner_process(command: &mut Command) {
    configure_background_process(command);
    configure_process_group(command);
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_process_tree_by_pid(pid: u32) -> LocalResult<()> {
    let process_group = i32::try_from(pid).map_err(|_| format!("Runner PID 无效: {pid}"))?;
    signal_process_group(process_group, libc::SIGTERM)?;
    thread::sleep(Duration::from_millis(200));
    signal_process_group(process_group, libc::SIGKILL)
}

#[cfg(unix)]
fn signal_process_group(process_group: i32, signal: i32) -> LocalResult<()> {
    let result = unsafe { libc::killpg(process_group, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(format!("无法终止 Runner 进程组 {process_group}: {error}"))
    }
}

#[cfg(windows)]
fn terminate_process_tree_by_pid(pid: u32) -> LocalResult<()> {
    let mut command = Command::new("taskkill");
    command.arg("/PID").arg(pid.to_string()).arg("/T").arg("/F");
    configure_background_process(&mut command);
    command
        .status()
        .map(|_| ())
        .map_err(|err| format!("无法终止 Runner 进程树 {pid}: {err}"))
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree_by_pid(pid: u32) -> LocalResult<()> {
    let mut system = System::new();
    let pid = Pid::from_u32(pid);
    system.refresh_process_specifics(pid, ProcessRefreshKind::everything());
    match system.process(pid) {
        Some(process) if process.kill() => Ok(()),
        Some(_) => Err(format!("无法终止 Runner 进程 {pid}")),
        None => Ok(()),
    }
}

pub fn terminate_runner(child: &mut Child) -> LocalResult<()> {
    let tree_result = terminate_process_tree_by_pid(child.id());
    let _ = child.kill();
    let wait_result = child
        .wait()
        .map(|_| ())
        .map_err(|err| format!("等待 Runner 进程退出失败: {err}"));
    tree_result.and(wait_result)
}

fn spawn_log_reader<R>(mut stream: R, tx: mpsc::Sender<Vec<u8>>)
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if tx.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> RunnerProcessIdentity {
        RunnerProcessIdentity {
            version: PROCESS_IDENTITY_VERSION,
            start_time: 123,
            executable: "/runtime/python".into(),
            command: vec![
                "/runtime/python".into(),
                "/app/runner.py".into(),
                "--job-dir".into(),
                "/data/jobs/job-1700000000000-1/attempts/attempt-2-3".into(),
            ],
        }
    }

    #[test]
    fn process_identity_rejects_pid_reuse_snapshot() {
        let expected = identity();
        let persisted = serde_json::to_string(&expected).unwrap();
        assert!(process_identity_matches(&persisted, &expected).unwrap());

        let mut reused = expected.clone();
        reused.start_time += 1;
        assert!(!process_identity_matches(&persisted, &reused).unwrap());
    }

    #[test]
    fn legacy_process_identity_requires_expected_attempt_directory() {
        let current = identity();
        assert!(process_identity_matches(
            "/runtime/python|/app/runner.py|job-1700000000000-1|2|3",
            &current,
        )
        .unwrap());
        assert!(!process_identity_matches(
            "/runtime/python|/app/runner.py|job-1700000000000-1|2|4",
            &current,
        )
        .unwrap());
    }
}
