use crate::local_db::{self, LocalResult, ManagedRuntimeState};
use chrono::Local;
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use sevenz_rust2::decompress_file;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};
use zip::ZipArchive;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static RUNTIME_INSTALLING: AtomicBool = AtomicBool::new(false);
const RUNTIME_MANIFEST_JSON: &str = include_str!("../resources/runtime-manifest.json");
const DOWNLOAD_RETRIES_PER_URL: usize = 3;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeManifest {
    runtime_version: String,
    python_version: String,
    pip_indexes: Vec<PipIndex>,
    platforms: Vec<PlatformRuntime>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PipIndex {
    label: String,
    url: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PlatformRuntime {
    platform_id: String,
    python_archive: DownloadAsset,
    python_executable_candidates: Vec<String>,
    ffmpeg_archive: Option<DownloadAsset>,
    #[serde(default)]
    ffmpeg_executable_candidates: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadAsset {
    sha256: String,
    urls: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedPythonRuntime {
    pub python_path: String,
    pub source_label: String,
    pub models_root: Option<String>,
    pub ffmpeg_path: Option<String>,
}

#[tauri::command]
pub fn get_runtime_status(app: AppHandle) -> LocalResult<ManagedRuntimeState> {
    detect_runtime_state(&app)
}

#[tauri::command]
pub fn install_runtime(app: AppHandle) -> LocalResult<ManagedRuntimeState> {
    let manifest = load_manifest()?;
    let platform_id = current_platform_id()?.to_string();

    let mut state = local_db::get_runtime_state(
        &app,
        &platform_id,
        &manifest.runtime_version,
        &manifest.python_version,
    )?;
    let log_path = runtime_log_path(&app, &platform_id)?;
    fs::create_dir_all(log_path.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|err| err.to_string())?;

    if RUNTIME_INSTALLING.swap(true, Ordering::SeqCst) {
        return detect_runtime_state(&app);
    }

    state.status = "installing".into();
    state.last_error = None;
    state.updated_at = unix_timestamp_millis().to_string();
    state.last_log_path = Some(log_path.to_string_lossy().into_owned());
    local_db::save_runtime_state(&app, &state)?;

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let result = perform_runtime_install(&app_handle);
        if let Err(error) = result {
            let _ = mark_install_failed(&app_handle, &error);
        }

        RUNTIME_INSTALLING.store(false, Ordering::SeqCst);
    });

    detect_runtime_state(&app)
}

#[tauri::command]
pub fn get_runtime_install_log(app: AppHandle) -> LocalResult<String> {
    let platform_id = current_platform_id()?;
    let log_path = runtime_log_path(&app, platform_id)?;
    Ok(fs::read_to_string(log_path).unwrap_or_default())
}

pub fn resolve_python_runtime(
    app: &AppHandle,
    manual_python_path: Option<&str>,
) -> LocalResult<ResolvedPythonRuntime> {
    let runtime_state = detect_runtime_state(app)?;

    if runtime_state.status == "ready" {
        if let Some(path) = runtime_state
            .python_executable_path
            .clone()
            .filter(|value| Path::new(value).is_file())
        {
            let ffmpeg_path = runtime_state
                .install_root
                .as_deref()
                .and_then(|value| resolve_managed_ffmpeg_path(Path::new(value)).ok())
                .flatten()
                .map(|value| value.to_string_lossy().into_owned());
            return Ok(ResolvedPythonRuntime {
                python_path: path,
                source_label: "managed Liberty runtime".into(),
                models_root: runtime_state.models_root.clone(),
                ffmpeg_path,
            });
        }
    }

    let manual = manual_python_path.unwrap_or("").trim();
    if !manual.is_empty() {
        if Path::new(manual).is_file() {
            return Ok(ResolvedPythonRuntime {
                python_path: manual.to_string(),
                source_label: "manual Python path".into(),
                models_root: None,
                ffmpeg_path: None,
            });
        }

        return Err("手动配置的 Python 路径不存在，请检查系统设置。".into());
    }

    Err("本地运行环境未安装，请前往系统设置下载并安装。".into())
}

fn detect_runtime_state(app: &AppHandle) -> LocalResult<ManagedRuntimeState> {
    let manifest = load_manifest()?;
    let platform_id = current_platform_id()?;
    let mut state = local_db::get_runtime_state(
        app,
        platform_id,
        &manifest.runtime_version,
        &manifest.python_version,
    )?;
    let mut changed = false;

    if state.runtime_version != manifest.runtime_version {
        state.runtime_version = manifest.runtime_version.clone();
        state.python_version = manifest.python_version.clone();
        if state.status == "ready" {
            state.status = "repair_required".into();
            state.last_error = Some("本地运行环境版本已更新，请重新安装。".into());
        }
        changed = true;
    }

    if state.status == "ready" {
        let platform = current_platform_manifest(&manifest)?;
        let python_missing = state
            .python_executable_path
            .as_deref()
            .map(Path::new)
            .map(|path| !path.is_file())
            .unwrap_or(true);
        let models_missing = state
            .models_root
            .as_deref()
            .map(Path::new)
            .map(|path| !path.is_dir())
            .unwrap_or(true);
        let ffmpeg_missing = if platform.ffmpeg_archive.is_some() {
            state
                .install_root
                .as_deref()
                .and_then(|value| resolve_managed_ffmpeg_path(Path::new(value)).ok())
                .flatten()
                .is_none()
        } else {
            false
        };

        if python_missing || models_missing || ffmpeg_missing {
            state.status = "repair_required".into();
            state.last_error = Some("本地运行环境不完整，请重新安装。".into());
            changed = true;
        }
    }

    if state.status == "installing" && !RUNTIME_INSTALLING.load(Ordering::SeqCst) {
        state.status = "failed".into();
        if state.last_error.is_none() {
            state.last_error = Some("上一次安装未完成，请重新安装。".into());
        }
        changed = true;
    }

    if changed {
        state.updated_at = unix_timestamp_millis().to_string();
        local_db::save_runtime_state(app, &state)?;
    }

    Ok(state)
}

fn perform_runtime_install(app: &AppHandle) -> LocalResult<()> {
    let manifest = load_manifest()?;
    let platform = current_platform_manifest(&manifest)?;
    let platform_id = platform.platform_id.clone();
    let runtime_root = runtime_platform_root(app, &platform_id)?;
    let downloads_root = runtime_root.join("downloads");
    let python_root = runtime_root.join("python");
    let ffmpeg_root = runtime_root.join("ffmpeg");
    let models_root = runtime_root.join("models");
    let log_path = runtime_log_path(app, &platform_id)?;
    let manifest_path = runtime_root.join("manifest.json");

    reset_runtime_workspace(
        &runtime_root,
        &downloads_root,
        &python_root,
        &ffmpeg_root,
        &models_root,
        &manifest_path,
    )?;
    append_install_log_line(
        &log_path,
        &format!(
            "[runtime] platform={} runtime_version={} python_version={}",
            platform_id, manifest.runtime_version, manifest.python_version
        ),
    )?;

    let archive_path = downloads_root.join("python-runtime.tar.gz");
    download_with_fallback(&platform.python_archive, &archive_path, &log_path)?;
    verify_sha256(&archive_path, &platform.python_archive.sha256, &log_path)?;
    extract_tar_gz(&archive_path, &runtime_root, &log_path)?;

    let python_executable = resolve_python_executable(&runtime_root, &platform)?;
    append_install_log_line(
        &log_path,
        &format!("[runtime] resolved python={}", python_executable.display()),
    )?;

    if let Some(ffmpeg_archive) = &platform.ffmpeg_archive {
        let ffmpeg_archive_path = if ffmpeg_archive
            .urls
            .first()
            .map(|value| value.to_ascii_lowercase().ends_with(".7z"))
            .unwrap_or(false)
        {
            downloads_root.join("ffmpeg-runtime.7z")
        } else {
            downloads_root.join("ffmpeg-runtime.zip")
        };
        download_with_fallback(ffmpeg_archive, &ffmpeg_archive_path, &log_path)?;
        verify_sha256(&ffmpeg_archive_path, &ffmpeg_archive.sha256, &log_path)?;
        extract_archive(&ffmpeg_archive_path, &ffmpeg_root, &log_path)?;
        if let Some(ffmpeg_executable) = resolve_ffmpeg_executable(&runtime_root, &platform) {
            append_install_log_line(
                &log_path,
                &format!("[runtime] resolved ffmpeg={}", ffmpeg_executable.display()),
            )?;
        } else {
            return Err("未找到托管运行环境中的 ffmpeg 可执行文件。".into());
        }
    }

    let requirements_path = resolve_script_resource_path(app, "runtime_requirements.txt")?;
    let warmup_path = resolve_script_resource_path(app, "runtime_warmup.py")?;
    install_python_dependencies(&python_executable, &requirements_path, &manifest.pip_indexes, &log_path)?;
    warmup_default_models(&python_executable, &warmup_path, &models_root, &log_path)?;

    let now = unix_timestamp_millis().to_string();
    let state = ManagedRuntimeState {
        platform_id,
        runtime_version: manifest.runtime_version,
        python_version: manifest.python_version,
        status: "ready".into(),
        python_executable_path: Some(python_executable.to_string_lossy().into_owned()),
        models_root: Some(models_root.to_string_lossy().into_owned()),
        install_root: Some(runtime_root.to_string_lossy().into_owned()),
        last_error: None,
        installed_at: Some(now.clone()),
        updated_at: now,
        last_log_path: Some(log_path.to_string_lossy().into_owned()),
    };
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&state).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    local_db::save_runtime_state(app, &state)?;
    append_install_log_line(&log_path, "[runtime] install completed.")?;
    Ok(())
}

fn mark_install_failed(app: &AppHandle, error: &str) -> LocalResult<()> {
    let manifest = load_manifest()?;
    let platform_id = current_platform_id()?.to_string();
    let log_path = runtime_log_path(app, &platform_id)?;
    append_install_log_line(&log_path, &format!("[runtime] install failed: {error}"))?;

    let mut state = local_db::get_runtime_state(
        app,
        &platform_id,
        &manifest.runtime_version,
        &manifest.python_version,
    )?;
    state.status = "failed".into();
    state.last_error = Some(error.to_string());
    state.updated_at = unix_timestamp_millis().to_string();
    state.last_log_path = Some(log_path.to_string_lossy().into_owned());
    local_db::save_runtime_state(app, &state)
}

fn install_python_dependencies(
    python_executable: &Path,
    requirements_path: &Path,
    pip_indexes: &[PipIndex],
    log_path: &Path,
) -> LocalResult<()> {
    bootstrap_pip(python_executable, log_path)?;
    upgrade_pip_tooling(python_executable, pip_indexes, log_path)?;

    let mut requirements_installed = false;
    let mut last_error: Option<String> = None;
    for index in pip_indexes {
        let install_result = run_command_with_log(
            Command::new(python_executable)
                .env("PYTHONUTF8", "1")
                .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
                .arg("-m")
                .arg("pip")
                .arg("install")
                .arg("--prefer-binary")
                .arg("--retries")
                .arg("2")
                .arg("--timeout")
                .arg("120")
                .arg("-r")
                .arg(requirements_path)
                .arg("-i")
                .arg(&index.url),
            log_path,
            &format!("Installing Python dependencies via {}", index.label),
        );
        if let Ok(()) = install_result {
            requirements_installed = true;
            break;
        }

        last_error = install_result.err();
    }

    if !requirements_installed {
        if let Some(error) = last_error {
            return Err(error);
        }

        return Err("Python 依赖安装失败。".into());
    }

    install_pytorch_stack(python_executable, pip_indexes, log_path)
}

fn install_pytorch_stack(
    python_executable: &Path,
    pip_indexes: &[PipIndex],
    log_path: &Path,
) -> LocalResult<()> {
    let mut install_plans: Vec<(String, Vec<String>)> = Vec::new();

    if cfg!(target_os = "windows") {
        install_plans.push((
            "Installing PyTorch CPU stack via PyTorch official index".into(),
            vec![
                "-m".into(),
                "pip".into(),
                "install".into(),
                "--force-reinstall".into(),
                "--no-cache-dir".into(),
                "--prefer-binary".into(),
                "--retries".into(),
                "2".into(),
                "--timeout".into(),
                "120".into(),
                "torch==2.2.2".into(),
                "torchvision==0.17.2".into(),
                "torchaudio==2.2.2".into(),
                "--index-url".into(),
                "https://download.pytorch.org/whl/cpu".into(),
            ],
        ));
    }

    for index in pip_indexes {
        install_plans.push((
            format!("Installing PyTorch stack via {}", index.label),
            vec![
                "-m".into(),
                "pip".into(),
                "install".into(),
                "--force-reinstall".into(),
                "--no-cache-dir".into(),
                "--prefer-binary".into(),
                "--retries".into(),
                "2".into(),
                "--timeout".into(),
                "120".into(),
                "torch==2.2.2".into(),
                "torchvision==0.17.2".into(),
                "torchaudio==2.2.2".into(),
                "-i".into(),
                index.url.clone(),
            ],
        ));
    }

    let mut last_error: Option<String> = None;
    for (description, args) in install_plans {
        let mut command = Command::new(python_executable);
        command.env("PYTHONUTF8", "1").env("PIP_DISABLE_PIP_VERSION_CHECK", "1");
        command.args(&args);

        let install_result = run_command_with_log(&mut command, log_path, &description);
        if let Ok(()) = install_result {
            return Ok(());
        }

        last_error = install_result.err();
    }

    Err(last_error.unwrap_or_else(|| "PyTorch 运行时安装失败。".into()))
}

fn bootstrap_pip(python_executable: &Path, log_path: &Path) -> LocalResult<()> {
    run_command_with_log(
        Command::new(python_executable)
            .env("PYTHONUTF8", "1")
            .arg("-m")
            .arg("ensurepip")
            .arg("--upgrade"),
        log_path,
        "Bootstrapping pip",
    )
}

fn upgrade_pip_tooling(
    python_executable: &Path,
    pip_indexes: &[PipIndex],
    log_path: &Path,
) -> LocalResult<()> {
    let mut last_error: Option<String> = None;

    for index in pip_indexes {
        let upgrade_result = run_command_with_log(
            Command::new(python_executable)
                .env("PYTHONUTF8", "1")
                .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
                .arg("-m")
                .arg("pip")
                .arg("install")
                .arg("--upgrade")
                .arg("pip")
                .arg("setuptools")
                .arg("wheel")
                .arg("--retries")
                .arg("2")
                .arg("--timeout")
                .arg("120")
                .arg("-i")
                .arg(&index.url),
            log_path,
            &format!("Upgrading pip via {}", index.label),
        );

        if let Ok(()) = upgrade_result {
            return Ok(());
        }

        last_error = upgrade_result.err();
    }

    Err(last_error.unwrap_or_else(|| "pip 工具升级失败。".into()))
}

fn warmup_default_models(
    python_executable: &Path,
    warmup_path: &Path,
    models_root: &Path,
    log_path: &Path,
) -> LocalResult<()> {
    let validate_path = warmup_path
        .parent()
        .map(|parent| parent.join("runtime_validate.py"))
        .ok_or_else(|| "未找到 runtime_validate.py 所在目录。".to_string())?;

    run_command_with_log(
        Command::new(python_executable)
            .env("PYTHONUTF8", "1")
            .arg(&validate_path),
        log_path,
        "Validating PyTorch runtime",
    )?;

    run_command_with_log(
        Command::new(python_executable)
            .env("PYTHONUTF8", "1")
            .env("MODELSCOPE_CACHE", models_root.join("modelscope"))
            .env("HF_HOME", models_root.join("huggingface"))
            .env("TORCH_HOME", models_root.join("torch"))
            .arg(warmup_path)
            .arg("--models-root")
            .arg(models_root),
        log_path,
        "Downloading default FunASR models",
    )
}

fn run_command_with_log(
    command: &mut Command,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    append_install_log_line(log_path, &format!("[runtime] {description}"))?;
    let output = command.output().map_err(|err| err.to_string())?;

    if !output.stdout.is_empty() {
        append_install_log(log_path, &output.stdout)?;
    }
    if !output.stderr.is_empty() {
        append_install_log(log_path, &output.stderr)?;
    }

    if output.status.success() {
        return Ok(());
    }

    Err(format!("{description} 失败，退出码 {}。", output.status.code().unwrap_or(-1)))
}

fn reset_runtime_workspace(
    runtime_root: &Path,
    downloads_root: &Path,
    python_root: &Path,
    ffmpeg_root: &Path,
    models_root: &Path,
    manifest_path: &Path,
) -> LocalResult<()> {
    fs::create_dir_all(runtime_root).map_err(|err| err.to_string())?;

    for path in [downloads_root, python_root, ffmpeg_root, models_root] {
        if path.exists() {
            fs::remove_dir_all(path).map_err(|err| err.to_string())?;
        }
    }

    if manifest_path.exists() {
        fs::remove_file(manifest_path).map_err(|err| err.to_string())?;
    }

    fs::create_dir_all(downloads_root).map_err(|err| err.to_string())?;
    fs::write(runtime_root.join("install.log"), []).map_err(|err| err.to_string())
}

fn download_with_fallback(asset: &DownloadAsset, target_path: &Path, log_path: &Path) -> LocalResult<()> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(900))
        .tcp_keepalive(Duration::from_secs(30))
        .no_proxy()
        .user_agent("Liberty Runtime Installer/2026.03")
        .build()
        .map_err(|err| err.to_string())?;
    let mut last_error = None;

    for url in &asset.urls {
        for attempt in 1..=DOWNLOAD_RETRIES_PER_URL {
            append_install_log_line(
                log_path,
                &format!(
                    "[runtime] downloading {url} (attempt {attempt}/{DOWNLOAD_RETRIES_PER_URL})"
                ),
            )?;
            match download_to_path(&client, url, target_path, log_path) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    append_install_log_line(
                        log_path,
                        &format!("[runtime] download failed: {error}"),
                    )?;
                    last_error = Some(error);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "运行时资源下载失败。".into()))
}

fn download_to_path(
    client: &Client,
    url: &str,
    target_path: &Path,
    log_path: &Path,
) -> LocalResult<()> {
    let temp_path = target_path.with_extension("download");
    let mut response = client
        .get(url)
        .send()
        .and_then(|value| value.error_for_status())
        .map_err(|err| err.to_string())?;
    let total_bytes = response.content_length();
    let _ = fs::remove_file(&temp_path);
    let _ = fs::remove_file(target_path);

    let mut target = File::create(&temp_path).map_err(|err| err.to_string())?;
    let mut buffer = vec![0u8; 512 * 1024];
    let mut downloaded_bytes: u64 = 0;
    let mut last_logged_bytes: u64 = 0;
    let mut last_log_at = Instant::now();

    if let Some(total) = total_bytes {
        append_install_log_line(
            log_path,
            &format!(
                "[runtime] download size {} MB from {}",
                bytes_to_mb(total),
                url
            ),
        )?;
    }

    loop {
        let read = response.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }

        target
            .write_all(&buffer[..read])
            .map_err(|err| err.to_string())?;
        downloaded_bytes += read as u64;

        let should_log = downloaded_bytes.saturating_sub(last_logged_bytes) >= 8 * 1024 * 1024
            || last_log_at.elapsed() >= Duration::from_secs(5);

        if should_log {
            match total_bytes {
                Some(total) if total > 0 => {
                    append_install_log_line(
                        log_path,
                        &format!(
                            "[runtime] download progress {} / {} MB ({:.1}%)",
                            bytes_to_mb(downloaded_bytes),
                            bytes_to_mb(total),
                            downloaded_bytes as f64 / total as f64 * 100.0
                        ),
                    )?;
                }
                _ => {
                    append_install_log_line(
                        log_path,
                        &format!(
                            "[runtime] download progress {} MB",
                            bytes_to_mb(downloaded_bytes)
                        ),
                    )?;
                }
            }

            last_logged_bytes = downloaded_bytes;
            last_log_at = Instant::now();
        }
    }

    target.flush().map_err(|err| err.to_string())?;
    target.sync_all().map_err(|err| err.to_string())?;
    fs::rename(&temp_path, target_path).map_err(|err| err.to_string())?;
    Ok(())
}

fn bytes_to_mb(value: u64) -> String {
    format!("{:.1}", value as f64 / 1024.0 / 1024.0)
}

fn verify_sha256(path: &Path, expected: &str, log_path: &Path) -> LocalResult<()> {
    append_install_log_line(log_path, "[runtime] verifying archive checksum")?;
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = format!("{:x}", hasher.finalize());
    if digest.eq_ignore_ascii_case(expected) {
        return Ok(());
    }

    Err(format!("运行时资源校验失败，期望 {expected}，实际 {digest}。"))
}

fn extract_archive(archive_path: &Path, destination: &Path, log_path: &Path) -> LocalResult<()> {
    let extension = archive_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "zip" => extract_zip(archive_path, destination, log_path),
        "7z" => extract_7z(archive_path, destination, log_path),
        _ => Err(format!(
            "不支持的 ffmpeg 压缩包格式：{}",
            archive_path.display()
        )),
    }
}

fn extract_zip(archive_path: &Path, destination: &Path, log_path: &Path) -> LocalResult<()> {
    append_install_log_line(log_path, "[runtime] extracting ffmpeg archive")?;
    fs::create_dir_all(destination).map_err(|err| err.to_string())?;
    let archive_file = File::open(archive_path).map_err(|err| err.to_string())?;
    let mut archive = ZipArchive::new(archive_file).map_err(|err| err.to_string())?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|err| err.to_string())?;
        let Some(entry_name) = entry.enclosed_name().map(|value| value.to_path_buf()) else {
            continue;
        };
        let output_path = destination.join(entry_name);

        if entry.name().ends_with('/') {
            fs::create_dir_all(&output_path).map_err(|err| err.to_string())?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }

        let mut output = File::create(&output_path).map_err(|err| err.to_string())?;
        std::io::copy(&mut entry, &mut output).map_err(|err| err.to_string())?;
        output.flush().map_err(|err| err.to_string())?;
        apply_zip_entry_permissions(&output_path, entry.unix_mode())?;
    }

    Ok(())
}

fn extract_7z(archive_path: &Path, destination: &Path, log_path: &Path) -> LocalResult<()> {
    append_install_log_line(log_path, "[runtime] extracting ffmpeg archive")?;
    fs::create_dir_all(destination).map_err(|err| err.to_string())?;
    decompress_file(archive_path, destination).map_err(|err| err.to_string())
}

fn apply_zip_entry_permissions(output_path: &Path, unix_mode: Option<u32>) -> LocalResult<()> {
    #[cfg(unix)]
    {
        if let Some(mode) = unix_mode {
            fs::set_permissions(output_path, fs::Permissions::from_mode(mode))
                .map_err(|err| err.to_string())?;
        }
    }

    #[cfg(not(unix))]
    {
        let _ = output_path;
        let _ = unix_mode;
    }

    Ok(())
}

fn extract_tar_gz(archive_path: &Path, destination: &Path, log_path: &Path) -> LocalResult<()> {
    append_install_log_line(log_path, "[runtime] extracting python runtime archive")?;
    let archive_file = File::open(archive_path).map_err(|err| err.to_string())?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(destination).map_err(|err| err.to_string())
}

fn resolve_python_executable(runtime_root: &Path, platform: &PlatformRuntime) -> LocalResult<PathBuf> {
    for candidate in &platform.python_executable_candidates {
        let path = runtime_root.join(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }

    Err("未找到托管运行环境中的 Python 可执行文件。".into())
}

fn resolve_ffmpeg_executable(runtime_root: &Path, platform: &PlatformRuntime) -> Option<PathBuf> {
    for candidate in &platform.ffmpeg_executable_candidates {
        let path = runtime_root.join(candidate);
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

fn resolve_managed_ffmpeg_path(runtime_root: &Path) -> LocalResult<Option<PathBuf>> {
    let manifest = load_manifest()?;
    let platform = current_platform_manifest(&manifest)?;
    let ffmpeg_path = resolve_ffmpeg_executable(runtime_root, &platform);
    if let Some(path) = ffmpeg_path.as_ref() {
        ensure_unix_executable(path)?;
    }
    Ok(ffmpeg_path)
}

fn ensure_unix_executable(path: &Path) -> LocalResult<()> {
    #[cfg(unix)]
    {
        let metadata = fs::metadata(path).map_err(|err| err.to_string())?;
        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        if mode & 0o111 == 0 {
            permissions.set_mode(mode | 0o755);
            fs::set_permissions(path, permissions).map_err(|err| err.to_string())?;
        }
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

fn resolve_script_resource_path(app: &AppHandle, file_name: &str) -> LocalResult<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join(file_name));
        candidates.push(resource_dir.join(file_name));
        candidates.push(resource_dir.join("_up_").join("scripts").join(file_name));
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("../scripts").join(file_name));
    candidates.push(manifest_dir.join("scripts").join(file_name));

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("scripts").join(file_name));
        candidates.push(current_dir.join("../scripts").join(file_name));
    }

    if let Ok(executable_path) = std::env::current_exe() {
        if let Some(executable_dir) = executable_path.parent() {
            candidates.push(executable_dir.join("scripts").join(file_name));
            candidates.push(executable_dir.join("../Resources/scripts").join(file_name));
            candidates.push(executable_dir.join("../Resources").join(file_name));
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate.canonicalize().unwrap_or(candidate));
        }
    }

    Err(format!("未找到内置脚本资源：{file_name}"))
}

fn load_manifest() -> LocalResult<RuntimeManifest> {
    serde_json::from_str(RUNTIME_MANIFEST_JSON).map_err(|err| err.to_string())
}

fn current_platform_manifest(manifest: &RuntimeManifest) -> LocalResult<PlatformRuntime> {
    let platform_id = current_platform_id()?;
    manifest
        .platforms
        .iter()
        .find(|item| item.platform_id == platform_id)
        .cloned()
        .ok_or_else(|| format!("暂不支持当前平台：{platform_id}"))
}

fn current_platform_id() -> LocalResult<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Ok("darwin-aarch64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Ok("darwin-x64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Ok("windows-x64")
    } else {
        Err("当前平台暂不支持托管本地运行环境。".into())
    }
}

fn runtime_platform_root(app: &AppHandle, platform_id: &str) -> LocalResult<PathBuf> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| err.to_string())?
        .join("runtime")
        .join(platform_id);
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    Ok(root)
}

fn runtime_log_path(app: &AppHandle, platform_id: &str) -> LocalResult<PathBuf> {
    Ok(runtime_platform_root(app, platform_id)?.join("install.log"))
}

fn append_install_log(log_path: &Path, bytes: &[u8]) -> LocalResult<()> {
    if bytes.is_empty() {
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|err| err.to_string())?;
    file.write_all(bytes).map_err(|err| err.to_string())
}

fn append_install_log_line(log_path: &Path, line: &str) -> LocalResult<()> {
    let prefix = runtime_log_prefix();
    append_install_log(log_path, format!("{prefix} {line}\n").as_bytes())
}

fn runtime_log_prefix() -> String {
    format!("[Liberty-下载进度-{}]", format_local_timestamp())
}

fn format_local_timestamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
