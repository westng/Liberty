mod archive;
mod logging;
mod manifest;
mod paths;
mod process;

use crate::local_db::{
    self, AppSettings, LocalResult, ManagedRuntimeState, RuntimeArtifactState,
    RuntimeComponentState, RuntimeOperationState,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tauri::AppHandle;

use archive::{download_remote_asset, extract_archive, verify_bundled_asset_sha256};
use logging::{
    append_install_log_line, runtime_component_generation_root, runtime_component_log_path,
    runtime_platform_root, unix_timestamp_millis,
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
use process::{
    run_command_with_log, run_command_with_log_timeout, validate_default_models_offline,
    validate_ffmpeg_runtime, warmup_default_models,
};

static PYTHON_BUSY: AtomicBool = AtomicBool::new(false);
static FFMPEG_BUSY: AtomicBool = AtomicBool::new(false);
static MODEL_BUSY: AtomicBool = AtomicBool::new(false);

const COMPONENT_PYTHON: &str = "python";
const COMPONENT_FFMPEG: &str = "ffmpeg";
const COMPONENT_MODEL: &str = "model";
const SOURCE_MANAGED: &str = "managed";
const SOURCE_SYSTEM: &str = "system";

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
    let settings = local_db::get_settings(&app)?;
    if settings.python_runtime_source == SOURCE_MANAGED {
        let _ = begin_component_install(&app, COMPONENT_PYTHON);
    }
    if settings.ffmpeg_runtime_source == SOURCE_MANAGED {
        let _ = begin_component_install(&app, COMPONENT_FFMPEG);
    }
    let _ = begin_component_install(&app, COMPONENT_MODEL);
    detect_runtime_state(&app)
}

#[tauri::command]
pub fn get_runtime_install_log(app: AppHandle) -> LocalResult<String> {
    let platform_id = current_platform_id()?;
    let mut sections = Vec::new();
    for component in [COMPONENT_PYTHON, COMPONENT_FFMPEG, COMPONENT_MODEL] {
        let log_path = runtime_component_log_path(&app, platform_id, component)?;
        let content = fs::read_to_string(log_path).unwrap_or_default();
        if !content.trim().is_empty() {
            sections.push(format!("[{component}]\n{content}"));
        }
    }
    Ok(sections.join("\n"))
}

#[tauri::command]
pub fn get_runtime_component_log(app: AppHandle, component: String) -> LocalResult<String> {
    let component = normalize_component(&component)?;
    let log_path = runtime_component_log_path(&app, current_platform_id()?, component)?;
    Ok(fs::read_to_string(log_path).unwrap_or_default())
}

#[tauri::command]
pub fn set_runtime_component_source(
    app: AppHandle,
    component: String,
    source: String,
) -> LocalResult<ManagedRuntimeState> {
    let component = normalize_selectable_component(&component)?;
    let source = normalize_source(&source)?;
    let mut settings = local_db::get_settings(&app)?;
    match component {
        COMPONENT_PYTHON => settings.python_runtime_source = source.into(),
        COMPONENT_FFMPEG => settings.ffmpeg_runtime_source = source.into(),
        _ => unreachable!(),
    }
    local_db::save_settings(&app, &settings)?;

    if source == SOURCE_SYSTEM {
        begin_component_detection(&app, component)?;
    } else {
        reconcile_managed_component(&app, component)?;
    }
    detect_runtime_state(&app)
}

#[tauri::command]
pub fn detect_runtime_component(
    app: AppHandle,
    component: String,
) -> LocalResult<ManagedRuntimeState> {
    let component = normalize_selectable_component(&component)?;
    let settings = local_db::get_settings(&app)?;
    if selected_source(&settings, component) != SOURCE_SYSTEM {
        return Err("只有选择本机环境后才能执行自动检测。".into());
    }
    begin_component_detection(&app, component)?;
    detect_runtime_state(&app)
}

#[tauri::command]
pub fn install_runtime_component(
    app: AppHandle,
    component: String,
) -> LocalResult<ManagedRuntimeState> {
    let component = normalize_component(&component)?;
    begin_component_install(&app, component)?;
    detect_runtime_state(&app)
}

pub fn resolve_python_runtime(
    app: &AppHandle,
    _settings: &AppSettings,
) -> LocalResult<ResolvedPythonRuntime> {
    let runtime_state = detect_runtime_state(app)?;
    let manifest = load_manifest()?;
    let platform = current_platform_manifest(&manifest)?;
    if !runtime_state.shell_ready {
        return Err("本地运行环境尚未就绪，请先完成 Python、FFmpeg 和模型配置。".into());
    }
    let python_path = active_component_path(&runtime_state.python)?;
    let ffmpeg_path = active_component_path(&runtime_state.ffmpeg)?;
    let models_root = active_component_path(&runtime_state.models)?;
    let source_label = if runtime_state.python.source.as_deref() == Some(SOURCE_SYSTEM) {
        "detected system Python"
    } else {
        "managed Liberty Python"
    };

    Ok(ResolvedPythonRuntime {
        python_path,
        source_label: source_label.into(),
        models_root: Some(models_root),
        ffmpeg_path: Some(ffmpeg_path),
        asr_backend: platform.asr_backend.unwrap_or_else(|| "funasr".into()),
    })
}

fn active_component_path(state: &RuntimeComponentState) -> LocalResult<String> {
    state
        .active_artifact
        .as_ref()
        .filter(|_| state.availability == "ready")
        .map(|artifact| artifact.resolved_path.clone())
        .ok_or_else(|| format!("{} 尚未就绪。", state.component))
}

fn normalize_component(component: &str) -> LocalResult<&'static str> {
    match component.trim() {
        COMPONENT_PYTHON => Ok(COMPONENT_PYTHON),
        COMPONENT_FFMPEG => Ok(COMPONENT_FFMPEG),
        COMPONENT_MODEL | "models" => Ok(COMPONENT_MODEL),
        _ => Err("不支持的运行环境组件。".into()),
    }
}

fn normalize_selectable_component(component: &str) -> LocalResult<&'static str> {
    match normalize_component(component)? {
        COMPONENT_PYTHON => Ok(COMPONENT_PYTHON),
        COMPONENT_FFMPEG => Ok(COMPONENT_FFMPEG),
        _ => Err("模型来源由 Liberty 托管，不能切换。".into()),
    }
}

fn normalize_source(source: &str) -> LocalResult<&'static str> {
    match source.trim() {
        SOURCE_MANAGED => Ok(SOURCE_MANAGED),
        SOURCE_SYSTEM => Ok(SOURCE_SYSTEM),
        _ => Err("不支持的运行环境来源。".into()),
    }
}

fn selected_source<'a>(settings: &'a AppSettings, component: &str) -> &'a str {
    match component {
        COMPONENT_PYTHON => &settings.python_runtime_source,
        COMPONENT_FFMPEG => &settings.ffmpeg_runtime_source,
        _ => SOURCE_MANAGED,
    }
}

fn component_busy_flag(component: &str) -> &'static AtomicBool {
    match component {
        COMPONENT_PYTHON => &PYTHON_BUSY,
        COMPONENT_FFMPEG => &FFMPEG_BUSY,
        _ => &MODEL_BUSY,
    }
}

fn detect_runtime_state(app: &AppHandle) -> LocalResult<ManagedRuntimeState> {
    let manifest = load_manifest()?;
    let platform_id = current_platform_id()?;
    if let Some(state) = unsupported_runtime_state(app, &manifest)? {
        return Ok(state);
    }
    let settings = local_db::get_settings(app)?;
    let mut state = local_db::get_runtime_state(
        app,
        platform_id,
        &manifest.runtime_version,
        &manifest.python_version,
    )?;
    let mut python = local_db::get_runtime_component_state(
        app,
        platform_id,
        COMPONENT_PYTHON,
        &settings.python_runtime_source,
    )?;
    let mut ffmpeg = local_db::get_runtime_component_state(
        app,
        platform_id,
        COMPONENT_FFMPEG,
        &settings.ffmpeg_runtime_source,
    )?;
    let mut models =
        local_db::get_runtime_component_state(app, platform_id, COMPONENT_MODEL, SOURCE_MANAGED)?;

    migrate_legacy_component_state(
        app,
        &manifest,
        &settings,
        &state,
        &mut python,
        &mut ffmpeg,
        &mut models,
    )?;
    reconcile_active_artifact(app, platform_id, &manifest, &mut python)?;
    reconcile_active_artifact(app, platform_id, &manifest, &mut ffmpeg)?;
    reconcile_active_artifact(app, platform_id, &manifest, &mut models)?;

    state.runtime_version = manifest.runtime_version.clone();
    state.python_version = manifest.python_version.clone();
    state.python_executable_path = active_path(&python);
    state.ffmpeg_path = active_path(&ffmpeg);
    state.models_root = active_path(&models);
    state.install_root = Some(
        runtime_platform_root(app, platform_id)?
            .to_string_lossy()
            .into_owned(),
    );
    state.python = python;
    state.ffmpeg = ffmpeg;
    state.models = models;
    state.shell_ready = [
        &state.python.availability,
        &state.ffmpeg.availability,
        &state.models.availability,
    ]
    .iter()
    .all(|availability| availability.as_str() == "ready");
    state.status = aggregate_runtime_status(&state).into();
    state.last_error = [&state.python, &state.ffmpeg, &state.models]
        .into_iter()
        .find_map(|component| component.operation.last_error.clone());
    state.updated_at = unix_timestamp_millis().to_string();
    local_db::save_runtime_state(app, &state)?;

    Ok(state)
}

pub fn detect_runtime_state_for_diagnostics(app: &AppHandle) -> LocalResult<ManagedRuntimeState> {
    detect_runtime_state(app)
}

fn active_path(state: &RuntimeComponentState) -> Option<String> {
    (state.availability == "ready")
        .then(|| {
            state
                .active_artifact
                .as_ref()
                .map(|artifact| artifact.resolved_path.clone())
        })
        .flatten()
}

fn aggregate_runtime_status(state: &ManagedRuntimeState) -> &'static str {
    if [&state.python, &state.ffmpeg, &state.models]
        .into_iter()
        .any(|component| component.availability == "unsupported")
    {
        return "unsupported";
    }
    if state.shell_ready {
        return "ready";
    }
    if [&state.python, &state.ffmpeg, &state.models]
        .into_iter()
        .any(|component| is_operation_active(&component.operation.kind))
    {
        return "installing";
    }
    if [&state.python, &state.ffmpeg, &state.models]
        .into_iter()
        .any(|component| component.operation.kind == "failed")
    {
        return "failed";
    }
    "missing"
}

fn is_operation_active(kind: &str) -> bool {
    matches!(
        kind,
        "detecting" | "waiting_for_python" | "downloading" | "installing" | "validating"
    )
}

fn migrate_legacy_component_state(
    app: &AppHandle,
    manifest: &RuntimeManifest,
    settings: &AppSettings,
    legacy: &ManagedRuntimeState,
    python: &mut RuntimeComponentState,
    ffmpeg: &mut RuntimeComponentState,
    models: &mut RuntimeComponentState,
) -> LocalResult<()> {
    let platform_id = current_platform_id()?;
    let platform = current_platform_manifest(manifest)?;
    if python.operation.generation == 0 && python.active_artifact.is_none() {
        let path = if settings.python_runtime_source == SOURCE_SYSTEM {
            PathBuf::from(settings.python_path.trim())
        } else {
            legacy
                .install_root
                .as_deref()
                .and_then(|root| resolve_python_executable(Path::new(root), &platform).ok())
                .or_else(|| legacy.python_executable_path.as_deref().map(PathBuf::from))
                .unwrap_or_default()
        };
        if path.is_file() {
            activate_legacy_component(
                python,
                if settings.python_runtime_source == SOURCE_SYSTEM {
                    "system"
                } else {
                    &manifest.runtime_version
                },
                &path,
            );
            local_db::save_runtime_component_state(app, platform_id, python)?;
        }
    }

    if ffmpeg.operation.generation == 0 && ffmpeg.active_artifact.is_none() {
        let path = if settings.ffmpeg_runtime_source == SOURCE_SYSTEM {
            PathBuf::from(settings.ffmpeg_path.trim())
        } else {
            legacy
                .install_root
                .as_deref()
                .and_then(|root| resolve_managed_ffmpeg_path(Path::new(root)).ok())
                .flatten()
                .unwrap_or_default()
        };
        if path.is_file() {
            activate_legacy_component(
                ffmpeg,
                if settings.ffmpeg_runtime_source == SOURCE_SYSTEM {
                    "system"
                } else {
                    &manifest.runtime_version
                },
                &path,
            );
            local_db::save_runtime_component_state(app, platform_id, ffmpeg)?;
        }
    }

    if models.operation.generation == 0 && models.active_artifact.is_none() {
        if let Some(path) = legacy.models_root.as_deref().map(PathBuf::from) {
            if path.is_dir() {
                activate_legacy_component(models, &manifest.model_set_version, &path);
                local_db::save_runtime_component_state(app, platform_id, models)?;
            }
        }
    }
    Ok(())
}

fn activate_legacy_component(
    state: &mut RuntimeComponentState,
    artifact_version: &str,
    path: &Path,
) {
    state.availability = "ready".into();
    state.active_artifact = Some(RuntimeArtifactState {
        generation_id: "legacy".into(),
        artifact_version: artifact_version.into(),
        resolved_path: path.to_string_lossy().into_owned(),
    });
    state.operation = RuntimeOperationState::default();
    state.updated_at = unix_timestamp_millis().to_string();
}

fn reconcile_active_artifact(
    app: &AppHandle,
    platform_id: &str,
    manifest: &RuntimeManifest,
    state: &mut RuntimeComponentState,
) -> LocalResult<()> {
    let mut changed = false;
    if let Some(artifact) = state.active_artifact.as_ref() {
        let path = Path::new(&artifact.resolved_path);
        let exists = if state.component == COMPONENT_MODEL {
            path.is_dir()
        } else {
            path.is_file()
        };
        let expected_version = if state.component == COMPONENT_MODEL {
            &manifest.model_set_version
        } else if state.source.as_deref() == Some(SOURCE_MANAGED) {
            &manifest.runtime_version
        } else {
            &artifact.artifact_version
        };
        if !exists || artifact.artifact_version != *expected_version {
            state.availability = "unavailable".into();
            state.active_artifact = None;
            state.operation.kind = "failed".into();
            state.operation.phase = if exists {
                "version_outdated".into()
            } else {
                "artifact_missing".into()
            };
            state.operation.progress = None;
            state.operation.last_error = Some(if exists {
                "运行组件版本已更新，请重新安装。".into()
            } else {
                "运行组件文件不存在，请重新安装或检测。".into()
            });
            changed = true;
        }
    }

    if is_operation_active(&state.operation.kind)
        && state.operation.kind != "waiting_for_python"
        && !component_busy_flag(&state.component).load(Ordering::SeqCst)
    {
        state.operation.kind = "failed".into();
        state.operation.phase = "interrupted".into();
        state.operation.progress = None;
        state.operation.last_error = Some("上一次组件操作未完成，请重试。".into());
        changed = true;
    }

    if changed {
        state.updated_at = unix_timestamp_millis().to_string();
        local_db::save_runtime_component_state(app, platform_id, state)?;
    }
    Ok(())
}

fn system_python_candidates() -> [&'static str; 8] {
    [
        "python3.13",
        "python3.12",
        "python3.11",
        "python3.10",
        "python3.9",
        "python3",
        "python",
        "py",
    ]
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

    #[cfg(windows)]
    {
        let versions = ["313", "312", "311", "310", "39"];
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            for version in versions {
                paths.push(
                    local_app_data
                        .join("Programs")
                        .join("Python")
                        .join(format!("Python{version}")),
                );
            }
        }
        if let Some(program_files) = env::var_os("ProgramFiles").map(PathBuf::from) {
            for version in versions {
                paths.push(program_files.join(format!("Python{version}")));
            }
            paths.push(program_files.join("ffmpeg").join("bin"));
        }
        if let Some(chocolatey) = env::var_os("ChocolateyInstall").map(PathBuf::from) {
            paths.push(chocolatey.join("bin"));
        }
        if let Some(user_profile) = env::var_os("USERPROFILE").map(PathBuf::from) {
            paths.push(user_profile.join("scoop").join("shims"));
        }
    }

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

fn begin_component_detection(app: &AppHandle, component: &str) -> LocalResult<()> {
    let component = normalize_selectable_component(component)?;
    let platform_id = current_platform_id()?;
    let mut state =
        local_db::get_runtime_component_state(app, platform_id, component, SOURCE_SYSTEM)?;
    let flag = component_busy_flag(component);
    if flag.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    state.operation.generation = state.operation.generation.saturating_add(1);
    state.operation.kind = "detecting".into();
    state.operation.phase = "scanning_system".into();
    state.operation.progress = None;
    state.operation.last_error = None;
    state.updated_at = unix_timestamp_millis().to_string();
    let generation = state.operation.generation;
    if let Err(error) = local_db::save_runtime_component_state(app, platform_id, &state) {
        flag.store(false, Ordering::SeqCst);
        return Err(error);
    }

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let result = perform_component_detection(&app_handle, component, generation);
        if let Err(error) = result {
            let _ = finish_component_failure(
                &app_handle,
                component,
                SOURCE_SYSTEM,
                generation,
                "system_not_detected",
                &error,
            );
        }
        flag.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn perform_component_detection(
    app: &AppHandle,
    component: &str,
    generation: u64,
) -> LocalResult<()> {
    let manifest = load_manifest()?;
    let platform = current_platform_manifest(&manifest)?;
    let platform_id = current_platform_id()?;
    let log_path = runtime_component_log_path(app, platform_id, component)?;
    fs::write(&log_path, []).map_err(|err| err.to_string())?;
    append_install_log_line(&log_path, "[runtime] scanning system environment")?;
    let settings = local_db::get_settings(app)?;

    let resolved = match component {
        COMPONENT_PYTHON => {
            let mut candidates = Vec::new();
            if !settings.python_path.trim().is_empty() {
                push_unique_path(&mut candidates, PathBuf::from(settings.python_path.trim()));
            }
            for path in resolve_system_executables(&system_python_candidates()) {
                push_unique_path(&mut candidates, path);
            }
            let backend = platform.asr_backend.as_deref().unwrap_or("funasr");
            let mut selected = None;
            for path in candidates {
                append_install_log_line(
                    &log_path,
                    &format!("[runtime] validating system python={}", path.display()),
                )?;
                if validate_python_runtime(app, &path, backend, &log_path).is_ok() {
                    selected = Some(path);
                    break;
                }
            }
            selected.ok_or_else(|| "未检测到满足本地转写依赖的本机 Python。".to_string())?
        }
        COMPONENT_FFMPEG => {
            let mut candidates = Vec::new();
            if !settings.ffmpeg_path.trim().is_empty() {
                push_unique_path(&mut candidates, PathBuf::from(settings.ffmpeg_path.trim()));
            }
            for path in resolve_system_executables(&["ffmpeg"]) {
                push_unique_path(&mut candidates, path);
            }
            let mut selected = None;
            for path in candidates {
                append_install_log_line(
                    &log_path,
                    &format!("[runtime] validating system ffmpeg={}", path.display()),
                )?;
                if validate_ffmpeg_runtime(&path, &log_path).is_ok() {
                    selected = Some(path);
                    break;
                }
            }
            selected.ok_or_else(|| "未检测到可用的本机 FFmpeg。".to_string())?
        }
        _ => return Err("模型不支持系统环境检测。".into()),
    };

    finish_system_detection_success(app, component, generation, &resolved)?;
    if component == COMPONENT_PYTHON {
        maybe_start_waiting_models(app)?;
    }
    Ok(())
}

fn finish_system_detection_success(
    app: &AppHandle,
    component: &str,
    generation: u64,
    path: &Path,
) -> LocalResult<()> {
    let platform_id = current_platform_id()?;
    let mut state =
        local_db::get_runtime_component_state(app, platform_id, component, SOURCE_SYSTEM)?;
    let settings = local_db::get_settings(app)?;
    if state.operation.generation != generation
        || selected_source(&settings, component) != SOURCE_SYSTEM
    {
        return Ok(());
    }
    state.availability = "ready".into();
    state.active_artifact = Some(RuntimeArtifactState {
        generation_id: format!("system-{generation}"),
        artifact_version: "system".into(),
        resolved_path: path.to_string_lossy().into_owned(),
    });
    state.operation.kind = "idle".into();
    state.operation.phase = "ready".into();
    state.operation.progress = Some(100);
    state.operation.last_error = None;
    state.updated_at = unix_timestamp_millis().to_string();
    local_db::save_runtime_component_state(app, platform_id, &state)?;

    let mut next_settings = settings;
    match component {
        COMPONENT_PYTHON => next_settings.python_path = path.to_string_lossy().into_owned(),
        COMPONENT_FFMPEG => next_settings.ffmpeg_path = path.to_string_lossy().into_owned(),
        _ => {}
    }
    local_db::save_settings(app, &next_settings)
}

fn begin_component_install(app: &AppHandle, component: &str) -> LocalResult<()> {
    let component = normalize_component(component)?;
    let manifest = load_manifest()?;
    let _ = selected_runtime_download_source(app, &manifest)?;
    let settings = local_db::get_settings(app)?;
    if component != COMPONENT_MODEL && selected_source(&settings, component) != SOURCE_MANAGED {
        return Err("选择本机环境时无需下载该组件，请执行重新检测。".into());
    }

    if component == COMPONENT_MODEL {
        let python = selected_python_state(app, &settings)?;
        if !model_can_start_with_python(&python) {
            let platform_id = current_platform_id()?;
            let mut models = local_db::get_runtime_component_state(
                app,
                platform_id,
                COMPONENT_MODEL,
                SOURCE_MANAGED,
            )?;
            if models.operation.kind != "waiting_for_python" {
                models.operation.generation = models.operation.generation.saturating_add(1);
            }
            models.operation.kind = "waiting_for_python".into();
            models.operation.phase = "waiting_for_python".into();
            models.operation.progress = None;
            models.operation.last_error = None;
            models.updated_at = unix_timestamp_millis().to_string();
            local_db::save_runtime_component_state(app, platform_id, &models)?;
            return Ok(());
        }
    }

    let platform_id = current_platform_id()?;
    let source = if component == COMPONENT_MODEL {
        SOURCE_MANAGED
    } else {
        selected_source(&settings, component)
    };
    let mut state = local_db::get_runtime_component_state(app, platform_id, component, source)?;
    let flag = component_busy_flag(component);
    if flag.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let generation = if state.operation.kind == "waiting_for_python" {
        state.operation.generation
    } else {
        state.operation.generation.saturating_add(1)
    };
    state.operation.generation = generation;
    state.operation.kind = if component == COMPONENT_MODEL {
        "installing".into()
    } else {
        "downloading".into()
    };
    state.operation.phase = if component == COMPONENT_MODEL {
        "acquiring_models".into()
    } else {
        "downloading".into()
    };
    state.operation.progress = None;
    state.operation.last_error = None;
    state.updated_at = unix_timestamp_millis().to_string();
    if let Err(error) = local_db::save_runtime_component_state(app, platform_id, &state) {
        flag.store(false, Ordering::SeqCst);
        return Err(error);
    }

    spawn_component_install(app.clone(), component, generation);
    Ok(())
}

fn spawn_component_install(app: AppHandle, component: &'static str, generation: u64) {
    let flag = component_busy_flag(component);
    std::thread::spawn(move || {
        let result = perform_component_install(&app, component, generation);
        if let Err(error) = result {
            let _ = finish_component_failure(
                &app,
                component,
                SOURCE_MANAGED,
                generation,
                "install_failed",
                &error,
            );
        }
        flag.store(false, Ordering::SeqCst);
    });
}

fn perform_component_install(app: &AppHandle, component: &str, generation: u64) -> LocalResult<()> {
    let manifest = load_manifest()?;
    let download_source = selected_runtime_download_source(app, &manifest)?;
    let platform = current_platform_manifest(&manifest)?;
    let platform_id = current_platform_id()?;
    let platform_root = runtime_platform_root(app, platform_id)?;
    let downloads_root = platform_root.join("downloads");
    fs::create_dir_all(&downloads_root).map_err(|err| err.to_string())?;
    let generation_root =
        runtime_component_generation_root(app, platform_id, component, generation)?;
    let log_path = runtime_component_log_path(app, platform_id, component)?;
    fs::write(&log_path, []).map_err(|err| err.to_string())?;
    append_runtime_header(&log_path, platform_id, &manifest)?;
    append_install_log_line(
        &log_path,
        &format!("[runtime] download source={}", download_source.name_zh),
    )?;

    let (resolved_path, artifact_version) = match component {
        COMPONENT_PYTHON => {
            update_component_phase(
                app,
                component,
                generation,
                "installing",
                "installing_python",
            )?;
            let executable = install_python_runtime(
                app,
                &platform,
                &download_source,
                platform_id,
                &generation_root,
                &downloads_root,
                &log_path,
            )?;
            (executable, manifest.runtime_version.clone())
        }
        COMPONENT_FFMPEG => {
            update_component_phase(
                app,
                component,
                generation,
                "installing",
                "installing_ffmpeg",
            )?;
            let executable = install_ffmpeg_runtime(
                &platform,
                &download_source,
                &generation_root,
                &downloads_root,
                &log_path,
            )?
            .ok_or_else(|| "当前平台没有可安装的 FFmpeg 组件。".to_string())?;
            (executable, manifest.runtime_version.clone())
        }
        COMPONENT_MODEL => {
            let settings = local_db::get_settings(app)?;
            let python = selected_python_state(app, &settings)?;
            let python_path = active_component_path(&python)?;
            let models_root = generation_root.join("models");
            fs::create_dir_all(&models_root).map_err(|err| err.to_string())?;
            let warmup_path = resolve_script_resource_path(app, "runtime_warmup.py")?;
            warmup_default_models(
                Path::new(&python_path),
                &warmup_path,
                &models_root,
                platform.asr_backend.as_deref().unwrap_or("funasr"),
                download_source.model_endpoint.as_deref(),
                &log_path,
            )?;
            update_component_phase(
                app,
                component,
                generation,
                "validating",
                "validating_models",
            )?;
            validate_default_models_offline(
                Path::new(&python_path),
                &warmup_path,
                &models_root,
                platform.asr_backend.as_deref().unwrap_or("funasr"),
                &log_path,
            )?;
            if !models_root.is_dir()
                || fs::read_dir(&models_root)
                    .map_err(|err| err.to_string())?
                    .next()
                    .is_none()
            {
                return Err("模型下载完成后未找到有效模型文件。".into());
            }
            (models_root, manifest.model_set_version.clone())
        }
        _ => return Err("不支持的运行环境组件。".into()),
    };

    write_component_marker(
        &generation_root,
        component,
        generation,
        &artifact_version,
        &resolved_path,
        &manifest.model_profile,
    )?;
    finish_managed_component_success(
        app,
        component,
        generation,
        &artifact_version,
        &resolved_path,
    )?;
    if component == COMPONENT_PYTHON {
        maybe_start_waiting_models(app)?;
    }
    Ok(())
}

fn selected_python_state(
    app: &AppHandle,
    settings: &AppSettings,
) -> LocalResult<RuntimeComponentState> {
    local_db::get_runtime_component_state(
        app,
        current_platform_id()?,
        COMPONENT_PYTHON,
        &settings.python_runtime_source,
    )
}

fn maybe_start_waiting_models(app: &AppHandle) -> LocalResult<()> {
    let platform_id = current_platform_id()?;
    let models =
        local_db::get_runtime_component_state(app, platform_id, COMPONENT_MODEL, SOURCE_MANAGED)?;
    if models.operation.kind != "waiting_for_python" {
        return Ok(());
    }
    let settings = local_db::get_settings(app)?;
    let python = selected_python_state(app, &settings)?;
    if model_can_start_with_python(&python) {
        begin_component_install(app, COMPONENT_MODEL)?;
    }
    Ok(())
}

fn model_can_start_with_python(python: &RuntimeComponentState) -> bool {
    python.availability == "ready" && python.active_artifact.is_some()
}

fn update_component_phase(
    app: &AppHandle,
    component: &str,
    generation: u64,
    operation_kind: &str,
    phase: &str,
) -> LocalResult<()> {
    let platform_id = current_platform_id()?;
    let mut state =
        local_db::get_runtime_component_state(app, platform_id, component, SOURCE_MANAGED)?;
    if state.operation.generation != generation {
        return Err("组件操作已被新的请求替代。".into());
    }
    state.operation.kind = operation_kind.into();
    state.operation.phase = phase.into();
    state.updated_at = unix_timestamp_millis().to_string();
    local_db::save_runtime_component_state(app, platform_id, &state)
}

fn finish_managed_component_success(
    app: &AppHandle,
    component: &str,
    generation: u64,
    artifact_version: &str,
    resolved_path: &Path,
) -> LocalResult<()> {
    let platform_id = current_platform_id()?;
    let settings = local_db::get_settings(app)?;
    if component != COMPONENT_MODEL && selected_source(&settings, component) != SOURCE_MANAGED {
        return Ok(());
    }
    let mut state =
        local_db::get_runtime_component_state(app, platform_id, component, SOURCE_MANAGED)?;
    if state.operation.generation != generation {
        return Ok(());
    }
    state.availability = "ready".into();
    state.active_artifact = Some(RuntimeArtifactState {
        generation_id: generation.to_string(),
        artifact_version: artifact_version.into(),
        resolved_path: resolved_path.to_string_lossy().into_owned(),
    });
    state.operation.kind = "idle".into();
    state.operation.phase = "ready".into();
    state.operation.progress = Some(100);
    state.operation.last_error = None;
    state.updated_at = unix_timestamp_millis().to_string();
    local_db::save_runtime_component_state(app, platform_id, &state)
}

fn finish_component_failure(
    app: &AppHandle,
    component: &str,
    source: &str,
    generation: u64,
    phase: &str,
    error: &str,
) -> LocalResult<()> {
    let platform_id = current_platform_id()?;
    let mut state = local_db::get_runtime_component_state(app, platform_id, component, source)?;
    if state.operation.generation != generation {
        return Ok(());
    }
    if source == SOURCE_SYSTEM {
        state.availability = "unavailable".into();
        state.active_artifact = None;
    }
    state.operation.kind = "failed".into();
    state.operation.phase = phase.into();
    state.operation.progress = None;
    state.operation.last_error = Some(error.into());
    state.updated_at = unix_timestamp_millis().to_string();
    local_db::save_runtime_component_state(app, platform_id, &state)
}

fn reconcile_managed_component(app: &AppHandle, component: &str) -> LocalResult<()> {
    let manifest = load_manifest()?;
    let platform_id = current_platform_id()?;
    let mut state =
        local_db::get_runtime_component_state(app, platform_id, component, SOURCE_MANAGED)?;
    reconcile_active_artifact(app, platform_id, &manifest, &mut state)
}

fn write_component_marker(
    generation_root: &Path,
    component: &str,
    generation: u64,
    artifact_version: &str,
    resolved_path: &Path,
    model_profile: &str,
) -> LocalResult<()> {
    let marker = serde_json::json!({
        "component": component,
        "generation": generation,
        "artifactVersion": artifact_version,
        "resolvedPath": resolved_path,
        "modelProfile": model_profile,
        "completedAt": unix_timestamp_millis().to_string(),
    });
    fs::write(
        generation_root.join("ready.json"),
        serde_json::to_vec_pretty(&marker).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
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
    let backend = platform.asr_backend.as_deref().unwrap_or("funasr");
    if let Ok(python_executable) = resolve_python_executable(runtime_root, platform) {
        ensure_unix_executable(&python_executable)?;
        append_install_log_line(
            log_path,
            &format!(
                "[runtime] reusing existing python={}",
                python_executable.display()
            ),
        )?;
        if validate_python_runtime(app, &python_executable, backend, log_path).is_ok() {
            return Ok(python_executable);
        }
        append_install_log_line(
            log_path,
            "[runtime] existing Python dependencies are incomplete, reinstalling dependencies",
        )?;
        install_python_dependencies(app, &python_executable, download_source, backend, log_path)?;
        validate_python_runtime(app, &python_executable, backend, log_path)?;
        return Ok(python_executable);
    }

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

    install_python_dependencies(app, &python_executable, download_source, backend, log_path)?;
    validate_python_runtime(app, &python_executable, backend, log_path)?;

    Ok(python_executable)
}

fn validate_python_runtime(
    app: &AppHandle,
    python_executable: &Path,
    backend: &str,
    log_path: &Path,
) -> LocalResult<()> {
    let validate_path = resolve_script_resource_path(app, "runtime_validate.py")?;
    run_command_with_log_timeout(
        Command::new(python_executable)
            .env("PYTHONUTF8", "1")
            .env("LIBERTY_ASR_BACKEND", backend)
            .arg(&validate_path),
        log_path,
        "Validating Python runtime",
        Duration::from_secs(90),
    )
}

fn install_ffmpeg_runtime(
    platform: &manifest::PlatformRuntime,
    download_source: &RuntimeDownloadSource,
    runtime_root: &Path,
    downloads_root: &Path,
    log_path: &Path,
) -> LocalResult<Option<PathBuf>> {
    let Some(ffmpeg_bundle) = &platform.ffmpeg_bundle else {
        return Ok(None);
    };

    if let Some(ffmpeg_executable) = resolve_ffmpeg_executable(runtime_root, platform) {
        ensure_unix_executable(&ffmpeg_executable)?;
        append_install_log_line(
            log_path,
            &format!(
                "[runtime] reusing existing ffmpeg={}",
                ffmpeg_executable.display()
            ),
        )?;
        if validate_ffmpeg_runtime(&ffmpeg_executable, log_path).is_ok() {
            return Ok(Some(ffmpeg_executable));
        }
        append_install_log_line(
            log_path,
            "[runtime] existing FFmpeg is not valid, reinstalling FFmpeg runtime",
        )?;
    }

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
    validate_ffmpeg_runtime(&ffmpeg_executable, log_path)?;
    Ok(Some(ffmpeg_executable))
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
    state.updated_at = now.clone();
    state.last_log_path = None;
    for component in [&mut state.python, &mut state.ffmpeg, &mut state.models] {
        component.availability = "unsupported".into();
        component.active_artifact = None;
        component.operation = RuntimeOperationState::default();
        component.updated_at = now.clone();
    }
    state.shell_ready = false;
    local_db::save_runtime_state(app, &state)?;
    Ok(Some(state))
}

#[cfg(test)]
mod tests {
    use super::{aggregate_runtime_status, model_can_start_with_python};
    use crate::local_db::{
        ManagedRuntimeState, RuntimeArtifactState, RuntimeComponentState, RuntimeOperationState,
    };

    #[test]
    fn model_uses_ready_python_even_while_python_redownloads() {
        let mut python = RuntimeComponentState::unavailable("python", Some("managed"));
        python.availability = "ready".into();
        python.active_artifact = Some(RuntimeArtifactState {
            generation_id: "4".into(),
            artifact_version: "runtime-4".into(),
            resolved_path: "/runtime/python4".into(),
        });
        python.operation = RuntimeOperationState {
            kind: "downloading".into(),
            generation: 5,
            phase: "downloading".into(),
            progress: Some(20),
            last_error: None,
        };

        assert!(model_can_start_with_python(&python));
    }

    #[test]
    fn aggregate_status_does_not_serialize_component_progress() {
        let mut state = ManagedRuntimeState::missing("darwin-aarch64", "runtime", "python");
        state.python.operation.kind = "downloading".into();
        state.ffmpeg.operation.kind = "installing".into();
        state.models.operation.kind = "waiting_for_python".into();

        assert_eq!(aggregate_runtime_status(&state), "installing");
        assert_eq!(state.python.operation.kind, "downloading");
        assert_eq!(state.ffmpeg.operation.kind, "installing");
        assert_eq!(state.models.operation.kind, "waiting_for_python");
    }
}
