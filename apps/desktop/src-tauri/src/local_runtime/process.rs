use std::{
    io::Read,
    path::Path,
    process::{Command, ExitStatus, Stdio},
    sync::mpsc,
};

use crate::local_db::LocalResult;
use crate::local_runtime::logging::{append_install_log, append_install_log_line};
use crate::process_utils::configure_background_process;

pub fn run_command_with_log(
    command: &mut Command,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    append_install_log_line(log_path, &format!("[runtime] {description}"))?;
    command
        .env("PYTHONUNBUFFERED", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_background_process(command);

    let mut child = command.spawn().map_err(|err| err.to_string())?;
    let status = stream_command_output(log_path, &mut child)?;

    if status.success() {
        return Ok(());
    }

    Err(format!(
        "{description} 失败，退出码 {}。",
        status.code().unwrap_or(-1)
    ))
}

fn stream_command_output(
    log_path: &Path,
    child: &mut std::process::Child,
) -> LocalResult<ExitStatus> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "运行时安装进程未返回 stdout 管道。".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "运行时安装进程未返回 stderr 管道。".to_string())?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    spawn_install_log_reader(stdout, tx.clone());
    spawn_install_log_reader(stderr, tx.clone());
    drop(tx);

    while let Ok(chunk) = rx.recv() {
        append_install_log(log_path, &chunk)?;
    }

    child.wait().map_err(|err| err.to_string())
}

fn spawn_install_log_reader<R>(mut stream: R, tx: mpsc::Sender<Vec<u8>>)
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

pub fn warmup_default_models(
    python_executable: &Path,
    warmup_path: &Path,
    models_root: &Path,
    asr_backend: &str,
    model_endpoint: Option<&str>,
    log_path: &Path,
) -> LocalResult<()> {
    let mut command = Command::new(python_executable);
    command
        .env("PYTHONUTF8", "1")
        .env("LIBERTY_ASR_BACKEND", asr_backend)
        .env("MODELSCOPE_CACHE", models_root.join("modelscope"))
        .env("HF_HOME", models_root.join("huggingface"))
        .env("TORCH_HOME", models_root.join("torch"))
        .arg(warmup_path)
        .arg("--models-root")
        .arg(models_root);

    if let Some(endpoint) = model_endpoint
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.env("MODELSCOPE_ENDPOINT", endpoint);
    }

    run_command_with_log(&mut command, log_path, "Downloading default ASR models")
}

pub fn validate_ffmpeg_runtime(ffmpeg_path: &Path, log_path: &Path) -> LocalResult<()> {
    run_command_with_log(
        Command::new(ffmpeg_path)
            .arg("-hide_banner")
            .arg("-version"),
        log_path,
        "Validating ffmpeg runtime",
    )
}
