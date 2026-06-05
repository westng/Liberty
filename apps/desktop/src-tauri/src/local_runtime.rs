mod archive;
mod logging;
mod manifest;
mod paths;
mod process;

use crate::local_db::{self, LocalResult, ManagedRuntimeState};
use std::{
    fs,
    path::Path,
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
    ensure_unix_executable, resolve_ffmpeg_executable, resolve_managed_ffmpeg_path,
    resolve_python_executable, resolve_script_resource_path,
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

#[tauri::command]
pub fn get_runtime_status(app: AppHandle) -> LocalResult<ManagedRuntimeState> {
    detect_runtime_state(&app)
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

fn asset_download_url(source: &RuntimeDownloadSource, asset: &BundledAsset) -> String {
    format!(
        "{}/{}",
        source.base_url.trim().trim_end_matches('/'),
        asset.file_name.trim_start_matches('/')
    )
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
    let python_download_url = asset_download_url(download_source, python_bundle);
    download_remote_asset(
        &python_download_url,
        &python_bundle_path,
        log_path,
        "downloading Python runtime",
    )?;
    verify_bundled_asset_sha256(&python_bundle_path, &python_bundle.sha256, log_path)?;
    extract_archive(
        &python_bundle_path,
        runtime_root,
        log_path,
        "extracting python runtime archive",
    )?;

    let python_executable = resolve_python_executable(runtime_root, platform)?;
    ensure_unix_executable(&python_executable)?;
    append_install_log_line(
        log_path,
        &format!("[runtime] resolved python={}", python_executable.display()),
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
    let ffmpeg_download_url = asset_download_url(download_source, ffmpeg_bundle);
    download_remote_asset(
        &ffmpeg_download_url,
        &ffmpeg_bundle_path,
        log_path,
        "downloading FFmpeg runtime",
    )?;
    verify_bundled_asset_sha256(&ffmpeg_bundle_path, &ffmpeg_bundle.sha256, log_path)?;
    extract_archive(
        &ffmpeg_bundle_path,
        runtime_root,
        log_path,
        "extracting ffmpeg archive",
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
        let models_download_url = asset_download_url(context.download_source, models_bundle);
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
