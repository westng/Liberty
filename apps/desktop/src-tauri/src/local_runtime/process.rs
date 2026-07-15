use std::{
    io::Read,
    path::Path,
    process::{Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crate::local_db::LocalResult;
use crate::local_runtime::logging::{append_install_log, append_install_log_line};
use crate::process_utils::configure_background_process;

pub fn run_command_with_log(
    command: &mut Command,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    run_command_with_log_timeout(command, log_path, description, Duration::from_secs(30 * 60))
}

pub fn run_command_with_log_timeout(
    command: &mut Command,
    log_path: &Path,
    description: &str,
    timeout: Duration,
) -> LocalResult<()> {
    append_install_log_line(log_path, &format!("[runtime] {description}"))?;
    command
        .env("PYTHONUNBUFFERED", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_background_process(command);
    configure_process_group(command);

    let mut child = command.spawn().map_err(|err| err.to_string())?;
    let status = stream_command_output(log_path, &mut child, timeout, description)?;

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
    timeout: Duration,
    description: &str,
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

    let started_at = Instant::now();
    let status = loop {
        for chunk in rx.try_iter() {
            append_install_log(log_path, &chunk)?;
        }
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            break status;
        }
        if started_at.elapsed() >= timeout {
            terminate_process_tree(child);
            child.wait().map_err(|err| err.to_string())?;
            for chunk in rx {
                append_install_log(log_path, &chunk)?;
            }
            return Err(format!(
                "{description} 超时（{} 秒），已终止进程。",
                timeout.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(50));
    };

    for chunk in rx {
        append_install_log(log_path, &chunk)?;
    }
    Ok(status)
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_process_tree(child: &mut std::process::Child) {
    let process_group = child.id() as i32;
    unsafe {
        libc::killpg(process_group, libc::SIGTERM);
    }
    thread::sleep(Duration::from_millis(200));
    if child.try_wait().ok().flatten().is_none() {
        unsafe {
            libc::killpg(process_group, libc::SIGKILL);
        }
    }
    let _ = child.kill();
}

#[cfg(windows)]
fn terminate_process_tree(child: &mut std::process::Child) {
    let mut command = Command::new("taskkill");
    command
        .arg("/PID")
        .arg(child.id().to_string())
        .arg("/T")
        .arg("/F");
    configure_background_process(&mut command);
    let _ = command.status();
    let _ = child.kill();
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree(child: &mut std::process::Child) {
    let _ = child.kill();
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

pub fn validate_default_models_offline(
    python_executable: &Path,
    warmup_path: &Path,
    models_root: &Path,
    asr_backend: &str,
    log_path: &Path,
) -> LocalResult<()> {
    let mut command = Command::new(python_executable);
    command
        .env("PYTHONUTF8", "1")
        .env("LIBERTY_ASR_BACKEND", asr_backend)
        .env("MODELSCOPE_CACHE", models_root.join("modelscope"))
        .env("HF_HOME", models_root.join("huggingface"))
        .env("TORCH_HOME", models_root.join("torch"))
        .env("MODELSCOPE_OFFLINE", "1")
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .arg(warmup_path)
        .arg("--models-root")
        .arg(models_root)
        .arg("--validate-only");

    run_command_with_log_timeout(
        &mut command,
        log_path,
        "Validating cached ASR models offline",
        Duration::from_secs(10 * 60),
    )
}

pub fn validate_ffmpeg_runtime(ffmpeg_path: &Path, log_path: &Path) -> LocalResult<()> {
    run_command_with_log_timeout(
        Command::new(ffmpeg_path)
            .arg("-hide_banner")
            .arg("-version"),
        log_path,
        "Validating ffmpeg runtime",
        Duration::from_secs(15),
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::run_command_with_log_timeout;
    use std::{
        fs,
        process::Command,
        time::{Duration, Instant},
    };

    #[test]
    fn timeout_terminates_process_group_without_waiting_for_descendants() {
        let log_path = std::env::temp_dir().join(format!(
            "liberty-runtime-timeout-{}.log",
            std::process::id()
        ));
        let started_at = Instant::now();
        let result = run_command_with_log_timeout(
            Command::new("sh").arg("-c").arg("sleep 5 & wait"),
            &log_path,
            "timeout test",
            Duration::from_millis(100),
        );

        assert!(result.expect_err("timeout").contains("超时"));
        assert!(started_at.elapsed() < Duration::from_secs(3));
        let _ = fs::remove_file(log_path);
    }
}
