mod archive;
mod logging;
mod manifest;
mod paths;
mod process;

use crate::local_db::{self, LocalResult, ManagedRuntimeState};
use crate::process_utils::configure_background_process;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::AppHandle;

use archive::{
    download_remote_asset, extract_archive, reset_runtime_workspace, verify_bundled_asset_sha256,
};
use logging::{
    append_install_log_line, runtime_log_path, runtime_platform_root, unix_timestamp_millis,
};
use manifest::{
    current_platform_id, current_platform_manifest, load_manifest, BundledAsset,
    RuntimeDownloadSource, RuntimeManifest,
};
use paths::{
    ensure_unix_executable, find_ffmpeg_executable, find_python_executable,
    resolve_ffmpeg_executable, resolve_managed_ffmpeg_path, resolve_python_executable,
    resolve_script_resource_path,
};
use process::{run_command_with_log, validate_ffmpeg_runtime, warmup_default_models};

static RUNTIME_INSTALLING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct ResolvedPythonRuntime {
    pub python_path: String,
    pub source_label: String,
    pub models_root: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub asr_backend: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDownloadSourceOption {
    pub id: String,
    pub label: String,
}

#[tauri::command]
pub fn get_runtime_status(app: AppHandle) -> LocalResult<ManagedRuntimeState> {
    detect_runtime_state(&app)
}

#[tauri::command]
pub fn list_runtime_download_sources() -> LocalResult<Vec<RuntimeDownloadSourceOption>> {
    let manifest = load_manifest()?;
    Ok(manifest
        .download_sources
        .into_iter()
        .map(|source| RuntimeDownloadSourceOption {
            id: source.source_id,
            label: source.name_zh,
        })
        .collect())
}

#[tauri::command]
pub fn install_runtime(app: AppHandle) -> LocalResult<ManagedRuntimeState> {
    let manifest = load_manifest()?;
    let platform_id = current_platform_id()?.to_string();
    if let Some(state) = unsupported_runtime_state(&app, &manifest)? {
        return Ok(state);
    }

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

#[tauri::command]
pub fn detect_system_runtime(app: AppHandle) -> LocalResult<ManagedRuntimeState> {
    detect_system_runtime_state(&app)
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
                ffmpeg_path: runtime_state.ffmpeg_path.clone().or(ffmpeg_path),
                asr_backend: current_runtime_backend(app).unwrap_or_else(|_| "funasr".into()),
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
                asr_backend: "funasr".into(),
            });
        }

        return Err("手动配置的 Python 路径不存在，请检查系统设置。".into());
    }

    if let Ok(system_state) = detect_system_runtime_state(app) {
        if system_state.status == "system_ready" {
            if let Some(path) = system_state
                .python_executable_path
                .clone()
                .filter(|value| Path::new(value).is_file())
            {
                return Ok(ResolvedPythonRuntime {
                    python_path: path,
                    source_label: "system runtime".into(),
                    models_root: system_state.models_root.clone(),
                    ffmpeg_path: system_state.ffmpeg_path.clone(),
                    asr_backend: current_runtime_backend(app).unwrap_or_else(|_| "funasr".into()),
                });
            }
        }
    }

    Err("本地运行环境未安装，请前往系统设置下载运行环境。".into())
}

fn detect_runtime_state(app: &AppHandle) -> LocalResult<ManagedRuntimeState> {
    let manifest = load_manifest()?;
    let platform_id = current_platform_id()?;
    if let Some(state) = unsupported_runtime_state(app, &manifest)? {
        return Ok(state);
    }

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

    if state.status == "ready" && runtime_artifacts_missing(&state, &manifest)? {
        state.status = "repair_required".into();
        state.last_error = Some("本地运行环境不完整，请重新安装。".into());
        changed = true;
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

pub fn detect_runtime_state_for_diagnostics(app: &AppHandle) -> LocalResult<ManagedRuntimeState> {
    detect_runtime_state(app)
}

fn detect_system_runtime_state(app: &AppHandle) -> LocalResult<ManagedRuntimeState> {
    let manifest = load_manifest()?;
    let platform_id = current_platform_id()?.to_string();
    let mut state = ManagedRuntimeState::missing(
        &platform_id,
        &manifest.runtime_version,
        &manifest.python_version,
    );
    let now = unix_timestamp_millis().to_string();
    state.updated_at = now;

    let validate_path = resolve_script_resource_path(app, "runtime_validate.py")?;
    let backend = current_platform_manifest(&manifest)?
        .asr_backend
        .unwrap_or_else(|| "funasr".into());
    let Some(python_path) = resolve_valid_system_python(&validate_path, &backend, &mut state)?
    else {
        if state.last_error.is_none() {
            state.last_error = Some("未检测到本机 Python。".into());
        }
        return Ok(state);
    };

    let Some(ffmpeg_path) = resolve_system_executable(&["ffmpeg"]) else {
        state.python_executable_path = Some(python_path.to_string_lossy().into_owned());
        state.last_error = Some("未检测到本机 FFmpeg。".into());
        return Ok(state);
    };

    let mut ffmpeg_command = Command::new(&ffmpeg_path);
    ffmpeg_command.arg("-hide_banner").arg("-version");
    configure_background_process(&mut ffmpeg_command);
    let ffmpeg_output = ffmpeg_command.output().map_err(|err| err.to_string())?;
    if !ffmpeg_output.status.success() {
        state.python_executable_path = Some(python_path.to_string_lossy().into_owned());
        state.ffmpeg_path = Some(ffmpeg_path.to_string_lossy().into_owned());
        state.last_error = Some("本机 FFmpeg 不可用。".into());
        return Ok(state);
    }

    state.status = "system_ready".into();
    state.python_executable_path = Some(python_path.to_string_lossy().into_owned());
    state.ffmpeg_path = Some(ffmpeg_path.to_string_lossy().into_owned());
    state.models_root =
        resolve_system_models_root().map(|path| path.to_string_lossy().into_owned());
    state.last_error = None;
    Ok(state)
}

fn resolve_valid_system_python(
    validate_path: &Path,
    backend: &str,
    state: &mut ManagedRuntimeState,
) -> LocalResult<Option<PathBuf>> {
    let python_candidates = resolve_system_executables(&system_python_candidates());
    if python_candidates.is_empty() {
        return Ok(None);
    }

    let mut last_error = String::new();
    for python_path in python_candidates {
        state.python_executable_path = Some(python_path.to_string_lossy().into_owned());
        let mut validate_command = Command::new(&python_path);
        validate_command
            .env("PYTHONUTF8", "1")
            .env("LIBERTY_ASR_BACKEND", backend)
            .arg(validate_path);
        configure_background_process(&mut validate_command);
        let validate_output = match validate_command.output() {
            Ok(output) => output,
            Err(error) => {
                last_error = format!("{} 启动失败：{error}", python_path.display());
                continue;
            }
        };

        if validate_output.status.success() {
            return Ok(Some(python_path));
        }

        let error_text = String::from_utf8_lossy(&validate_output.stderr)
            .trim()
            .to_string();
        last_error = if error_text.is_empty() {
            format!("{} 验证未通过。", python_path.display())
        } else {
            format!("{}：{error_text}", python_path.display())
        };
    }

    state.last_error = Some(if last_error.is_empty() {
        "本机 Python 缺少本地转写依赖。".into()
    } else {
        format!("本机 Python 缺少本地转写依赖：{last_error}")
    });
    Ok(None)
}

fn system_python_candidates() -> [&'static str; 3] {
    ["python3", "python", "py"]
}

fn resolve_system_executable(candidates: &[&str]) -> Option<PathBuf> {
    resolve_system_executables(candidates).into_iter().next()
}

fn resolve_system_executables(candidates: &[&str]) -> Vec<PathBuf> {
    let mut resolved = Vec::new();
    for candidate in candidates {
        let candidate_path = Path::new(candidate);
        if candidate_path.is_file() {
            push_unique_path(&mut resolved, candidate_path.to_path_buf());
        }

        for dir in system_search_paths() {
            let path = dir.join(candidate);
            if path.is_file() {
                push_unique_path(&mut resolved, path);
            }

            #[cfg(windows)]
            {
                let exe_path = dir.join(format!("{candidate}.exe"));
                if exe_path.is_file() {
                    push_unique_path(&mut resolved, exe_path);
                }
            }
        }
    }

    resolved
}

fn system_search_paths() -> Vec<PathBuf> {
    let mut paths = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();

    #[cfg(target_os = "macos")]
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
        PathBuf::from("/opt/local/bin"),
    ]);

    #[cfg(all(unix, not(target_os = "macos")))]
    paths.extend([
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]);

    let mut unique = Vec::new();
    for path in paths {
        push_unique_path(&mut unique, path);
    }
    unique
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn resolve_system_models_root() -> Option<PathBuf> {
    env::var_os("LIBERTY_MODELS_ROOT")
        .map(PathBuf::from)
        .filter(|path| path.exists())
}

fn runtime_artifacts_missing(
    state: &ManagedRuntimeState,
    manifest: &RuntimeManifest,
) -> LocalResult<bool> {
    let platform = current_platform_manifest(manifest)?;
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
    let ffmpeg_missing = if platform.ffmpeg_bundle.is_some() {
        state
            .install_root
            .as_deref()
            .and_then(|value| resolve_managed_ffmpeg_path(Path::new(value)).ok())
            .flatten()
            .is_none()
    } else {
        false
    };

    Ok(python_missing || models_missing || ffmpeg_missing)
}

fn perform_runtime_install(app: &AppHandle) -> LocalResult<()> {
    let manifest = load_manifest()?;
    let download_source = selected_runtime_download_source(app, &manifest)?;
    let platform = current_platform_manifest(&manifest)?;
    let platform_id = platform.platform_id.clone();
    let runtime_root = runtime_platform_root(app, &platform_id)?;
    let downloads_root = runtime_root.join("downloads");
    let python_root = runtime_root.join("python");
    let ffmpeg_root = runtime_root.join("ffmpeg");
    let models_root = runtime_root.join("models");
    let log_path = runtime_log_path(app, &platform_id)?;
    let manifest_path = runtime_root.join("manifest.json");
    let had_models = models_root.is_dir();

    reset_runtime_workspace(
        &runtime_root,
        &downloads_root,
        &python_root,
        &ffmpeg_root,
        &manifest_path,
    )?;
    append_runtime_header(&log_path, &platform_id, &manifest)?;
    append_install_log_line(
        &log_path,
        &format!("[runtime] download source={}", download_source.name_zh),
    )?;

    let python_executable = install_python_runtime(
        app,
        &platform,
        &download_source,
        &platform_id,
        &runtime_root,
        &downloads_root,
        &log_path,
    )?;
    install_ffmpeg_runtime(
        &platform,
        &download_source,
        &runtime_root,
        &downloads_root,
        &log_path,
    )?;
    install_or_reuse_models(ModelInstallContext {
        app,
        platform: &platform,
        download_source: &download_source,
        runtime_root: &runtime_root,
        downloads_root: &downloads_root,
        models_root: &models_root,
        python_executable: &python_executable,
        had_models,
        log_path: &log_path,
    })?;

    if !models_root.is_dir() {
        return Err("未找到托管运行环境中的本地 ASR 模型目录。".into());
    }

    let state = build_ready_runtime_state(
        platform_id,
        manifest,
        &python_executable,
        &models_root,
        &runtime_root,
        &log_path,
    );
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&state).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    local_db::save_runtime_state(app, &state)?;
    append_install_log_line(&log_path, "[runtime] install completed.")?;
    Ok(())
}

fn selected_runtime_download_source(
    app: &AppHandle,
    manifest: &RuntimeManifest,
) -> LocalResult<RuntimeDownloadSource> {
    let settings = local_db::get_settings(app)?;
    let source_id = settings.runtime_download_source.trim();
    if manifest.download_sources.is_empty() {
        return Err("运行环境下载源未配置，请先配置真实可用的下载源。".into());
    }
    if source_id.is_empty() {
        return Err("请选择下载源后再下载运行环境。".into());
    }

    manifest
        .download_sources
        .iter()
        .find(|source| source.source_id == source_id)
        .cloned()
        .ok_or_else(|| format!("下载源配置不存在，请重新选择下载源：{source_id}"))
}

fn asset_download_url(source: &RuntimeDownloadSource, asset: &BundledAsset) -> LocalResult<String> {
    if let Some(url) = asset
        .urls
        .get(&source.source_id)
        .or_else(|| asset.urls.get("official"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return Ok(url.to_string());
    }

    Err(format!(
        "运行资源缺少可用下载地址：{} / {}",
        source.name_zh, asset.file_name
    ))
}

fn append_runtime_header(
    log_path: &Path,
    platform_id: &str,
    manifest: &RuntimeManifest,
) -> LocalResult<()> {
    append_install_log_line(
        log_path,
        &format!(
            "[runtime] platform={} runtime_version={} python_version={}",
            platform_id, manifest.runtime_version, manifest.python_version
        ),
    )?;
    append_install_log_line(log_path, "[runtime] locating remote runtime resources")
}

fn install_python_runtime(
    app: &AppHandle,
    platform: &manifest::PlatformRuntime,
    download_source: &RuntimeDownloadSource,
    platform_id: &str,
    runtime_root: &Path,
    downloads_root: &Path,
    log_path: &Path,
) -> LocalResult<std::path::PathBuf> {
    let python_bundle = platform
        .python_bundle
        .as_ref()
        .ok_or_else(|| format!("当前平台缺少远程 Python 运行时配置：{platform_id}"))?;
    let python_bundle_path = downloads_root.join(&python_bundle.file_name);
    let python_download_url = asset_download_url(download_source, python_bundle)?;
    download_remote_asset(
        &python_download_url,
        &python_bundle_path,
        log_path,
        "downloading Python runtime",
    )?;
    verify_bundled_asset_sha256(&python_bundle_path, &python_bundle.sha256, log_path)?;
    extract_asset_to_runtime_dir(
        &python_bundle_path,
        runtime_root,
        "python",
        log_path,
        "extracting python runtime archive",
        find_python_executable,
    )?;

    let python_executable = resolve_python_executable(runtime_root, platform)?;
    ensure_unix_executable(&python_executable)?;
    append_install_log_line(
        log_path,
        &format!("[runtime] resolved python={}", python_executable.display()),
    )?;

    install_python_dependencies(
        app,
        &python_executable,
        download_source,
        platform.asr_backend.as_deref().unwrap_or("funasr"),
        log_path,
    )?;

    let validate_path = resolve_script_resource_path(app, "runtime_validate.py")?;
    run_command_with_log(
        Command::new(&python_executable)
            .env("PYTHONUTF8", "1")
            .env(
                "LIBERTY_ASR_BACKEND",
                platform.asr_backend.as_deref().unwrap_or("funasr"),
            )
            .arg(&validate_path),
        log_path,
        "Validating Python runtime",
    )?;

    Ok(python_executable)
}

fn install_ffmpeg_runtime(
    platform: &manifest::PlatformRuntime,
    download_source: &RuntimeDownloadSource,
    runtime_root: &Path,
    downloads_root: &Path,
    log_path: &Path,
) -> LocalResult<()> {
    let Some(ffmpeg_bundle) = &platform.ffmpeg_bundle else {
        return Ok(());
    };

    let ffmpeg_bundle_path = downloads_root.join(&ffmpeg_bundle.file_name);
    let ffmpeg_download_url = asset_download_url(download_source, ffmpeg_bundle)?;
    download_remote_asset(
        &ffmpeg_download_url,
        &ffmpeg_bundle_path,
        log_path,
        "downloading FFmpeg runtime",
    )?;
    verify_bundled_asset_sha256(&ffmpeg_bundle_path, &ffmpeg_bundle.sha256, log_path)?;
    extract_asset_to_runtime_dir(
        &ffmpeg_bundle_path,
        runtime_root,
        "ffmpeg",
        log_path,
        "extracting ffmpeg archive",
        find_ffmpeg_executable,
    )?;

    let ffmpeg_executable = resolve_ffmpeg_executable(runtime_root, platform)
        .ok_or_else(|| "未找到托管运行环境中的 ffmpeg 可执行文件。".to_string())?;
    ensure_unix_executable(&ffmpeg_executable)?;
    append_install_log_line(
        log_path,
        &format!("[runtime] resolved ffmpeg={}", ffmpeg_executable.display()),
    )?;
    validate_ffmpeg_runtime(&ffmpeg_executable, log_path)
}

fn extract_asset_to_runtime_dir(
    archive_path: &Path,
    runtime_root: &Path,
    target_dir_name: &str,
    log_path: &Path,
    description: &str,
    find_marker: fn(&Path) -> Option<PathBuf>,
) -> LocalResult<()> {
    let stage_dir = runtime_root.join(format!("{target_dir_name}.stage"));
    let target_dir = runtime_root.join(target_dir_name);
    if stage_dir.exists() {
        fs::remove_dir_all(&stage_dir).map_err(|err| err.to_string())?;
    }
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).map_err(|err| err.to_string())?;
    }

    extract_archive(archive_path, &stage_dir, log_path, description)?;
    let marker = find_marker(&stage_dir).ok_or_else(|| {
        format!(
            "未在上游压缩包中找到 {} 可执行文件：{}",
            target_dir_name,
            archive_path.display()
        )
    })?;
    let source_root = choose_asset_root(&stage_dir, &marker);
    if fs::rename(&source_root, &target_dir).is_err() {
        copy_dir_all(&source_root, &target_dir)?;
        fs::remove_dir_all(&source_root).map_err(|err| err.to_string())?;
    }
    if stage_dir.exists() {
        let _ = fs::remove_dir_all(&stage_dir);
    }
    Ok(())
}

fn choose_asset_root(stage_dir: &Path, marker: &Path) -> PathBuf {
    let mut current = marker.parent().unwrap_or(stage_dir);
    while current.parent().is_some_and(|parent| parent != stage_dir) {
        current = current.parent().unwrap_or(current);
    }

    if current == stage_dir {
        marker.parent().unwrap_or(stage_dir).to_path_buf()
    } else {
        current.to_path_buf()
    }
}

fn copy_dir_all(source: &Path, target: &Path) -> LocalResult<()> {
    fs::create_dir_all(target).map_err(|err| err.to_string())?;
    for entry in fs::read_dir(source).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

fn install_python_dependencies(
    app: &AppHandle,
    python_executable: &Path,
    download_source: &RuntimeDownloadSource,
    backend: &str,
    log_path: &Path,
) -> LocalResult<()> {
    let requirements_path = resolve_script_resource_path(app, "requirements.txt")?;
    let mut command = Command::new(python_executable);
    command
        .env("PYTHONUTF8", "1")
        .env("LIBERTY_ASR_BACKEND", backend)
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--disable-pip-version-check")
        .arg("--no-input");
    if let Some(index_url) = download_source
        .pip_index_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.arg("-i").arg(index_url);
    }
    command.arg("-r").arg(requirements_path);
    run_command_with_log(
        &mut command,
        log_path,
        "Installing Python runtime dependencies",
    )
}

struct ModelInstallContext<'a> {
    app: &'a AppHandle,
    platform: &'a manifest::PlatformRuntime,
    download_source: &'a RuntimeDownloadSource,
    runtime_root: &'a Path,
    downloads_root: &'a Path,
    models_root: &'a Path,
    python_executable: &'a Path,
    had_models: bool,
    log_path: &'a Path,
}

fn install_or_reuse_models(context: ModelInstallContext<'_>) -> LocalResult<()> {
    if let Some(models_bundle) = &context.platform.models_bundle {
        let models_bundle_path = context.downloads_root.join(&models_bundle.file_name);
        let models_download_url = asset_download_url(context.download_source, models_bundle)?;
        download_remote_asset(
            &models_download_url,
            &models_bundle_path,
            context.log_path,
            "downloading ASR models bundle",
        )?;
        verify_bundled_asset_sha256(&models_bundle_path, &models_bundle.sha256, context.log_path)?;
        extract_archive(
            &models_bundle_path,
            context.runtime_root,
            context.log_path,
            "extracting models archive",
        )?;
        return append_install_log_line(
            context.log_path,
            "[runtime] validating remote models root",
        );
    }

    if context.had_models {
        return append_install_log_line(context.log_path, "[runtime] reusing existing ASR models");
    }

    let backend = context.platform.asr_backend.as_deref().unwrap_or("funasr");
    let warmup_path = resolve_script_resource_path(context.app, "runtime_warmup.py")?;
    warmup_default_models(
        context.python_executable,
        &warmup_path,
        context.models_root,
        backend,
        context.download_source.model_endpoint.as_deref(),
        context.log_path,
    )
}

fn build_ready_runtime_state(
    platform_id: String,
    manifest: RuntimeManifest,
    python_executable: &Path,
    models_root: &Path,
    runtime_root: &Path,
    log_path: &Path,
) -> ManagedRuntimeState {
    let now = unix_timestamp_millis().to_string();
    ManagedRuntimeState {
        platform_id,
        runtime_version: manifest.runtime_version,
        python_version: manifest.python_version,
        status: "ready".into(),
        python_executable_path: Some(python_executable.to_string_lossy().into_owned()),
        models_root: Some(models_root.to_string_lossy().into_owned()),
        install_root: Some(runtime_root.to_string_lossy().into_owned()),
        ffmpeg_path: resolve_managed_ffmpeg_path(runtime_root)
            .ok()
            .flatten()
            .map(|path| path.to_string_lossy().into_owned()),
        last_error: None,
        installed_at: Some(now.clone()),
        updated_at: now,
        last_log_path: Some(log_path.to_string_lossy().into_owned()),
    }
}

fn current_runtime_backend(_app: &AppHandle) -> LocalResult<String> {
    let manifest = load_manifest()?;
    let platform = current_platform_manifest(&manifest)?;
    Ok(platform.asr_backend.unwrap_or_else(|| "funasr".into()))
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

fn unsupported_runtime_state(
    app: &AppHandle,
    manifest: &RuntimeManifest,
) -> LocalResult<Option<ManagedRuntimeState>> {
    let platform = current_platform_manifest(manifest)?;
    let Some(reason) = platform.unsupported_reason.as_ref() else {
        return Ok(None);
    };

    let now = unix_timestamp_millis().to_string();
    let mut state = local_db::get_runtime_state(
        app,
        &platform.platform_id,
        &manifest.runtime_version,
        &manifest.python_version,
    )?;
    state.runtime_version = manifest.runtime_version.clone();
    state.python_version = manifest.python_version.clone();
    state.status = "unsupported".into();
    state.python_executable_path = None;
    state.models_root = None;
    state.install_root = None;
    state.last_error = Some(reason.clone());
    state.installed_at = None;
    state.updated_at = now;
    state.last_log_path = None;
    local_db::save_runtime_state(app, &state)?;
    Ok(Some(state))
}
