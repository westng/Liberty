use std::{
    io::Read,
    path::Path,
    process::{Child, Command, ExitStatus},
    sync::mpsc,
    time::{Duration, Instant},
};

use tauri::AppHandle;

use crate::{
    infrastructure::process_logs,
    local_db::{AppSettings, LocalResult, MeetingJob},
    local_runtime::ResolvedPythonRuntime,
    process_utils::configure_background_process,
};

const MAX_LOCAL_ASR_THREADS: u32 = 32;

pub struct RunnerCommandInput<'a> {
    pub runtime: &'a ResolvedPythonRuntime,
    pub settings: &'a AppSettings,
    pub job: &'a MeetingJob,
    pub job_dir: &'a Path,
    pub runner_script_path: &'a Path,
    pub input_file: &'a Path,
    pub runtime_threads: u32,
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

pub fn stream_child_logs<F>(
    app: &AppHandle,
    job_id: &str,
    job_dir: &Path,
    child: &mut Child,
    mut sync_log: F,
) -> LocalResult<ExitStatus>
where
    F: FnMut(&AppHandle, &str, &Path) -> LocalResult<()>,
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
    while let Ok(chunk) = rx.recv() {
        process_logs::append_bytes(job_dir, &chunk)?;
        if last_sync_at.elapsed() >= Duration::from_millis(400) {
            sync_log(app, job_id, job_dir)?;
            last_sync_at = Instant::now();
        }
    }

    let status = child.wait().map_err(|err| err.to_string())?;
    sync_log(app, job_id, job_dir)?;
    Ok(status)
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
