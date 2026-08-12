mod archive;
mod logging;
mod manifest;
mod paths;
mod process;

use crate::{
    application::switch_runtime_source::{self, RuntimeSourcePort},
    domain::runtime::{
        install_components, selected_source as selected_runtime_source, RuntimeComponent,
        RuntimeSource,
    },
    local_db::{
        self, AppSettings, LocalResult, ManagedRuntimeState, RuntimeArtifactState,
        RuntimeComponentState, RuntimeOperationState,
    },
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env, fs,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
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
use process::{run_command_with_log, run_command_with_log_timeout, validate_ffmpeg_runtime};

static PYTHON_BUSY: AtomicBool = AtomicBool::new(false);
static FFMPEG_BUSY: AtomicBool = AtomicBool::new(false);
static MODEL_BUSY: AtomicBool = AtomicBool::new(false);

const COMPONENT_PYTHON: &str = "python";
const COMPONENT_FFMPEG: &str = "ffmpeg";
const COMPONENT_MODEL: &str = "model";
const SOURCE_MANAGED: &str = "managed";
const SOURCE_SYSTEM: &str = "system";
const MODELS_ACQUIRED_MARKER: &str = "models-acquired.json";
const MODEL_DOWNLOAD_PROGRESS_BYTES: u64 = 64 * 1024 * 1024;
const MODELSCOPE_CACHE_METADATA_SCRIPT: &str = r#"
import json
import os
import pickle
import sys
import tempfile

marker_path, models_root = sys.argv[1:3]
with open(marker_path, "r", encoding="utf-8") as stream:
    marker = json.load(stream)

def atomic_pickle(path, value):
    fd, temp_path = tempfile.mkstemp(dir=os.path.dirname(path))
    try:
        with os.fdopen(fd, "wb") as stream:
            pickle.dump(value, stream)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp_path, path)
    except BaseException:
        try:
            os.unlink(temp_path)
        except OSError:
            pass
        raise

for model in marker["models"]:
    model_root = os.path.join(models_root, *model["relativePath"].split("/"))
    atomic_pickle(
        os.path.join(model_root, ".msc"),
        [{"Path": item["path"], "Revision": item["revision"]} for item in model["files"]],
    )
    atomic_pickle(os.path.join(model_root, ".mdl"), {"id": model["modelId"]})
"#;

const DEFAULT_FUNASR_MODELS: [(&str, &str, &str); 4] = [
    (
        "model",
        "iic/speech_seaco_paraformer_large_asr_nat-zh-cn-16k-common-vocab8404-pytorch",
        "0141367fdc9b6ba58b0442ef34bceb56a6c1789c",
    ),
    (
        "vad",
        "iic/speech_fsmn_vad_zh-cn-16k-common-pytorch",
        "f9a8b8274674755d925277e27063869038d41515",
    ),
    (
        "punc",
        "iic/punc_ct-transformer_cn-en-common-vocab471067-large",
        "45ab6961ad58a973ce7785401b4e93a0aab907a3",
    ),
    (
        "spk",
        "iic/speech_campplus_sv_zh-cn_16k-common",
        "a045b2afcaa9c3049c98a9215a2bc274407ab237",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelInstallAction {
    Acquire,
    WaitForPython,
    Validate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcquiredModelSet {
    model_set_version: String,
    model_profile: String,
    models: Vec<AcquiredModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcquiredModel {
    role: String,
    model_id: String,
    revision: String,
    relative_path: String,
    files: Vec<AcquiredModelFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcquiredModelFile {
    path: String,
    revision: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ModelScopeFilesResponse {
    success: bool,
    code: i64,
    message: String,
    data: ModelScopeFilesData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ModelScopeFilesData {
    files: Vec<ModelScopeFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ModelScopeFile {
    path: String,
    revision: String,
    sha256: String,
    size: u64,
    #[serde(rename = "Type")]
    file_type: String,
}

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
    for component in runtime_install_components(&settings) {
        let _ = begin_component_install(&app, component);
    }
    detect_runtime_state(&app)
}

fn runtime_install_components(settings: &AppSettings) -> Vec<&'static str> {
    install_components(settings)
        .into_iter()
        .map(RuntimeComponent::as_str)
        .collect()
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
    switch_runtime_source::switch_runtime_source(
        &LocalRuntimeSourcePort { app: &app },
        &component,
        &source,
    )
}

struct LocalRuntimeSourcePort<'a> {
    app: &'a AppHandle,
}

impl RuntimeSourcePort for LocalRuntimeSourcePort<'_> {
    type State = ManagedRuntimeState;

    fn set_source(&self, component: RuntimeComponent, source: RuntimeSource) -> LocalResult<()> {
        local_db::set_runtime_component_source(self.app, component.as_str(), source.as_str())
            .map(|_| ())
    }

    fn detect_system(&self, component: RuntimeComponent) -> LocalResult<()> {
        begin_component_detection(self.app, component.as_str())
    }

    fn reconcile_managed(&self, component: RuntimeComponent) -> LocalResult<()> {
        reconcile_managed_component(self.app, component.as_str())
    }

    fn load_state(&self) -> LocalResult<Self::State> {
        detect_runtime_state(self.app)
    }
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
    RuntimeComponent::parse(component).map(RuntimeComponent::as_str)
}

fn normalize_selectable_component(component: &str) -> LocalResult<&'static str> {
    RuntimeComponent::parse_selectable(component).map(RuntimeComponent::as_str)
}

fn selected_source<'a>(settings: &'a AppSettings, component: &str) -> &'a str {
    RuntimeComponent::parse(component)
        .map(|component| selected_runtime_source(settings, component))
        .unwrap_or(SOURCE_MANAGED)
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
    if state.models.operation.kind == "waiting_for_python"
        && model_can_start_with_python(&state.python)
    {
        let _ = begin_component_install(app, COMPONENT_MODEL);
    }

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
    if state.operation.generation != generation {
        return Ok(());
    }
    let resolved_path = path.to_string_lossy().into_owned();
    state.availability = "ready".into();
    state.active_artifact = Some(RuntimeArtifactState {
        generation_id: format!("system-{generation}"),
        artifact_version: "system".into(),
        resolved_path: resolved_path.clone(),
    });
    state.operation.kind = "idle".into();
    state.operation.phase = "ready".into();
    state.operation.progress = Some(100);
    state.operation.last_error = None;
    state.updated_at = unix_timestamp_millis().to_string();
    local_db::publish_detected_runtime_path(
        app,
        platform_id,
        &state,
        &resolved_path,
        SOURCE_SYSTEM,
        generation,
    )
    .map(|_| ())
}

fn begin_component_install(app: &AppHandle, component: &str) -> LocalResult<()> {
    let component = normalize_component(component)?;
    let manifest = load_manifest()?;
    let settings = local_db::get_settings(app)?;
    if component != COMPONENT_MODEL && selected_source(&settings, component) != SOURCE_MANAGED {
        return Err("选择本机环境时无需下载该组件，请执行重新检测。".into());
    }

    let platform_id = current_platform_id()?;
    let source = if component == COMPONENT_MODEL {
        SOURCE_MANAGED
    } else {
        selected_source(&settings, component)
    };
    let mut state = local_db::get_runtime_component_state(app, platform_id, component, source)?;
    let flag = component_busy_flag(component);
    if flag.load(Ordering::SeqCst) {
        return Ok(());
    }
    let can_retry_model_generation = component == COMPONENT_MODEL
        && state.operation.kind == "failed"
        && model_generation_can_retry(app, platform_id, state.operation.generation, &manifest);
    let generation = if component == COMPONENT_MODEL
        && (state.operation.kind == "waiting_for_python" || can_retry_model_generation)
    {
        state.operation.generation
    } else {
        state.operation.generation.saturating_add(1)
    };
    let action = if component == COMPONENT_MODEL {
        let generation_root = model_generation_path(app, platform_id, generation)?;
        model_install_action(
            &generation_root,
            &manifest,
            &selected_python_state(app, &settings)?,
        )
    } else {
        ModelInstallAction::Acquire
    };
    if action == ModelInstallAction::WaitForPython {
        if state.operation.kind != "waiting_for_python" {
            state.operation.generation = generation;
            state.operation.kind = "waiting_for_python".into();
            state.operation.phase = "waiting_for_python".into();
            state.operation.progress = Some(100);
            state.operation.last_error = None;
            state.updated_at = unix_timestamp_millis().to_string();
            local_db::save_runtime_component_state(app, platform_id, &state)?;
            maybe_start_waiting_models(app)?;
        }
        return Ok(());
    }
    if component != COMPONENT_MODEL || action == ModelInstallAction::Acquire {
        let _ = selected_runtime_download_source(app, &manifest)?;
    }

    if flag.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    state.operation.generation = generation;
    state.operation.kind = if component == COMPONENT_MODEL && action == ModelInstallAction::Validate
    {
        "validating".into()
    } else {
        "downloading".into()
    };
    state.operation.phase =
        if component == COMPONENT_MODEL && action == ModelInstallAction::Validate {
            "validating_models".into()
        } else if component == COMPONENT_MODEL {
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

    spawn_component_install(app.clone(), component, generation, action);
    Ok(())
}

fn spawn_component_install(
    app: AppHandle,
    component: &'static str,
    generation: u64,
    action: ModelInstallAction,
) {
    let flag = component_busy_flag(component);
    std::thread::spawn(move || {
        let result = perform_component_install(&app, component, generation, action);
        if let Err(error) = result {
            let failure_phase = match (component, action) {
                (COMPONENT_MODEL, ModelInstallAction::Acquire) => "model_acquisition_failed",
                (COMPONENT_MODEL, ModelInstallAction::Validate) => "model_validation_failed",
                _ => "install_failed",
            };
            let _ = finish_component_failure(
                &app,
                component,
                SOURCE_MANAGED,
                generation,
                failure_phase,
                &error,
            );
        }
        flag.store(false, Ordering::SeqCst);
        if component == COMPONENT_MODEL {
            let _ = maybe_start_waiting_models(&app);
        }
    });
}

fn perform_component_install(
    app: &AppHandle,
    component: &str,
    generation: u64,
    action: ModelInstallAction,
) -> LocalResult<()> {
    let manifest = load_manifest()?;
    let download_source = if component != COMPONENT_MODEL || action == ModelInstallAction::Acquire {
        Some(selected_runtime_download_source(app, &manifest)?)
    } else {
        None
    };
    let platform = current_platform_manifest(&manifest)?;
    let platform_id = current_platform_id()?;
    let platform_root = runtime_platform_root(app, platform_id)?;
    let downloads_root = platform_root.join("downloads");
    if component != COMPONENT_MODEL {
        fs::create_dir_all(&downloads_root).map_err(|err| err.to_string())?;
    }
    let generation_root =
        runtime_component_generation_root(app, platform_id, component, generation)?;
    let log_path = runtime_component_log_path(app, platform_id, component)?;
    if component != COMPONENT_MODEL || action == ModelInstallAction::Acquire || !log_path.is_file()
    {
        fs::write(&log_path, []).map_err(|err| err.to_string())?;
    }
    append_runtime_header(&log_path, platform_id, &manifest)?;
    if let Some(download_source) = download_source.as_ref() {
        append_install_log_line(
            &log_path,
            &format!("[runtime] download source={}", download_source.name_zh),
        )?;
    } else {
        append_install_log_line(
            &log_path,
            "[runtime] model snapshots already acquired; validating offline",
        )?;
    }

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
                download_source
                    .as_ref()
                    .ok_or_else(|| "Python 安装缺少下载源。".to_string())?,
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
                download_source
                    .as_ref()
                    .ok_or_else(|| "FFmpeg 安装缺少下载源。".to_string())?,
                &generation_root,
                &downloads_root,
                &log_path,
            )?
            .ok_or_else(|| "当前平台没有可安装的 FFmpeg 组件。".to_string())?;
            (executable, manifest.runtime_version.clone())
        }
        COMPONENT_MODEL => {
            let models_root = generation_root.join("models");
            fs::create_dir_all(&models_root).map_err(|err| err.to_string())?;
            if action == ModelInstallAction::Acquire {
                acquire_default_models(
                    &manifest,
                    download_source
                        .as_ref()
                        .ok_or_else(|| "模型获取缺少下载源。".to_string())?,
                    &models_root,
                    &generation_root,
                    &log_path,
                )?;
            }
            let settings = local_db::get_settings(app)?;
            let python = selected_python_state(app, &settings)?;
            if !model_can_start_with_python(&python) {
                persist_models_waiting_for_python(app, generation)?;
                return Ok(());
            }
            update_component_phase(
                app,
                component,
                generation,
                "validating",
                "validating_models",
            )?;
            validate_acquired_models(
                app,
                Path::new(&active_component_path(&python)?),
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

fn model_install_action(
    generation_root: &Path,
    manifest: &RuntimeManifest,
    python: &RuntimeComponentState,
) -> ModelInstallAction {
    if !models_acquired(generation_root, manifest) {
        return ModelInstallAction::Acquire;
    }
    if model_can_start_with_python(python) {
        ModelInstallAction::Validate
    } else {
        ModelInstallAction::WaitForPython
    }
}

fn models_acquired(generation_root: &Path, manifest: &RuntimeManifest) -> bool {
    read_acquired_model_set(generation_root).is_some_and(|set| {
        validate_acquired_model_set(generation_root, manifest, &set, None).is_ok()
    })
}

fn model_generation_can_retry(
    app: &AppHandle,
    platform_id: &str,
    generation: u64,
    manifest: &RuntimeManifest,
) -> bool {
    model_generation_path(app, platform_id, generation)
        .ok()
        .is_some_and(|generation_root| {
            read_acquired_model_set(&generation_root).map_or_else(
                || generation_root.join("models").is_dir(),
                |set| validate_acquired_model_metadata(manifest, &set).is_ok(),
            )
        })
}

fn model_generation_path(
    app: &AppHandle,
    platform_id: &str,
    generation: u64,
) -> LocalResult<PathBuf> {
    Ok(runtime_platform_root(app, platform_id)?
        .join("components")
        .join(COMPONENT_MODEL)
        .join("generations")
        .join(generation.to_string()))
}

fn read_acquired_model_set(generation_root: &Path) -> Option<AcquiredModelSet> {
    fs::read(generation_root.join(MODELS_ACQUIRED_MARKER))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn acquire_default_models(
    manifest: &RuntimeManifest,
    download_source: &RuntimeDownloadSource,
    models_root: &Path,
    generation_root: &Path,
    log_path: &Path,
) -> LocalResult<()> {
    if manifest.model_profile != "paraformer" {
        return Err(format!(
            "当前模型获取器不支持模型配置：{}",
            manifest.model_profile
        ));
    }
    let endpoint = download_source
        .model_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "当前下载源没有配置 ModelScope 地址。".to_string())?;
    if !endpoint.to_ascii_lowercase().starts_with("https://") {
        return Err("ModelScope 地址必须使用 HTTPS。".into());
    }
    let acquired_marker = generation_root.join(MODELS_ACQUIRED_MARKER);
    if acquired_marker.exists() {
        fs::remove_file(&acquired_marker).map_err(|err| err.to_string())?;
    }

    append_install_log_line(
        log_path,
        "[runtime] acquiring default FunASR model snapshots",
    )?;
    let client = Client::builder()
        .timeout(Duration::from_secs(60 * 30))
        .build()
        .map_err(|err| err.to_string())?;
    let mut acquired_models = Vec::with_capacity(DEFAULT_FUNASR_MODELS.len());

    for (role, model_id, revision) in DEFAULT_FUNASR_MODELS {
        let relative_path = model_cache_relative_path(model_id)?;
        let model_root = models_root.join(&relative_path);
        let files = fetch_modelscope_files(&client, endpoint, model_id, revision)?;
        append_install_log_line(
            log_path,
            &format!(
                "[runtime] acquiring model role={role} id={model_id} files={}",
                files.len()
            ),
        )?;
        fs::create_dir_all(&model_root).map_err(|err| err.to_string())?;

        let mut acquired_files = Vec::with_capacity(files.len());
        for file in files {
            let relative_file_path = safe_model_relative_path(&file.path)?;
            let target_path = model_root.join(&relative_file_path);
            download_modelscope_file(&client, endpoint, model_id, &file, &target_path, log_path)?;
            acquired_files.push(AcquiredModelFile {
                path: path_to_forward_slashes(&relative_file_path)?,
                revision: file.revision,
                sha256: file.sha256.to_ascii_lowercase(),
                size: file.size,
            });
        }

        acquired_models.push(AcquiredModel {
            role: role.into(),
            model_id: model_id.into(),
            revision: revision.into(),
            relative_path: path_to_forward_slashes(&relative_path)?,
            files: acquired_files,
        });
    }

    let acquired_set = AcquiredModelSet {
        model_set_version: manifest.model_set_version.clone(),
        model_profile: manifest.model_profile.clone(),
        models: acquired_models,
    };
    validate_acquired_model_set(generation_root, manifest, &acquired_set, None)?;
    write_json_atomically(&acquired_marker, &acquired_set)?;
    append_install_log_line(log_path, "[runtime] default model snapshots acquired")
}

fn fetch_modelscope_files(
    client: &Client,
    endpoint: &str,
    model_id: &str,
    revision: &str,
) -> LocalResult<Vec<ModelScopeFile>> {
    validate_model_id(model_id)?;
    validate_model_revision(revision)?;
    let url = format!(
        "{}/api/v1/models/{model_id}/repo/files",
        endpoint.trim_end_matches('/')
    );
    let response = client
        .get(url)
        .query(&[("Revision", revision), ("Recursive", "true")])
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<ModelScopeFilesResponse>()
        .map_err(|err| err.to_string())?;
    if !response.success || response.code != 200 {
        return Err(format!(
            "ModelScope 模型清单请求失败：{} ({})",
            response.message, response.code
        ));
    }

    let mut seen_paths = HashSet::new();
    let mut files = Vec::new();
    for file in response.data.files {
        if file.file_type != "blob" {
            continue;
        }
        let path = safe_model_relative_path(&file.path)?;
        let normalized_path = path_to_forward_slashes(&path)?;
        if !seen_paths.insert(normalized_path.clone()) {
            return Err(format!(
                "ModelScope 模型清单包含重复文件：{normalized_path}"
            ));
        }
        validate_model_revision(&file.revision).map_err(|error| {
            format!("ModelScope 模型文件 revision 无效：{normalized_path}: {error}")
        })?;
        if !valid_sha256(&file.sha256) {
            return Err(format!(
                "ModelScope 模型文件缺少有效 SHA-256：{normalized_path}"
            ));
        }
        files.push(ModelScopeFile {
            path: normalized_path,
            revision: file.revision,
            sha256: file.sha256.to_ascii_lowercase(),
            size: file.size,
            file_type: file.file_type,
        });
    }
    if files.is_empty() {
        return Err(format!("ModelScope 模型清单没有可下载文件：{model_id}"));
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn download_modelscope_file(
    client: &Client,
    endpoint: &str,
    model_id: &str,
    model_file: &ModelScopeFile,
    target_path: &Path,
    log_path: &Path,
) -> LocalResult<()> {
    if target_path.is_file() && verify_model_file_size(target_path, model_file.size).is_ok() {
        match verify_bundled_asset_sha256(target_path, &model_file.sha256, log_path) {
            Ok(()) => {
                append_install_log_line(
                    log_path,
                    &format!("[runtime] reusing verified model file {}", model_file.path),
                )?;
                return Ok(());
            }
            Err(error) => append_install_log_line(
                log_path,
                &format!(
                    "[runtime] cached model file rejected {}: {error}",
                    model_file.path
                ),
            )?,
        }
    }
    if target_path.exists() {
        fs::remove_file(target_path).map_err(|err| err.to_string())?;
    }
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let url = format!(
        "{}/api/v1/models/{model_id}/repo",
        endpoint.trim_end_matches('/')
    );
    append_install_log_line(
        log_path,
        &format!(
            "[runtime] downloading model file {} ({} MB)",
            model_file.path,
            bytes_to_mb(model_file.size)
        ),
    )?;
    let mut response = client
        .get(url)
        .query(&[
            ("Revision", model_file.revision.as_str()),
            ("FilePath", model_file.path.as_str()),
        ])
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?;
    if let Some(content_length) = response.content_length() {
        if content_length != model_file.size {
            return Err(format!(
                "模型文件大小与清单不一致：{}，期望 {}，响应 {}。",
                model_file.path, model_file.size, content_length
            ));
        }
    }

    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("模型文件缺少有效文件名：{}", model_file.path))?;
    let temp_path = target_path.with_file_name(format!("{file_name}.download"));
    let _ = fs::remove_file(&temp_path);
    let mut target = File::create(&temp_path).map_err(|err| err.to_string())?;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut downloaded_bytes = 0u64;
    let mut last_logged_bytes = 0u64;
    let mut last_log_at = Instant::now();
    let download_result = (|| -> LocalResult<()> {
        loop {
            let read = response.read(&mut buffer).map_err(|err| err.to_string())?;
            if read == 0 {
                break;
            }
            target
                .write_all(&buffer[..read])
                .map_err(|err| err.to_string())?;
            downloaded_bytes = downloaded_bytes.saturating_add(read as u64);
            if downloaded_bytes.saturating_sub(last_logged_bytes) >= MODEL_DOWNLOAD_PROGRESS_BYTES
                || last_log_at.elapsed() >= Duration::from_secs(5)
            {
                append_install_log_line(
                    log_path,
                    &format!(
                        "[runtime] model file progress {} / {} MB",
                        bytes_to_mb(downloaded_bytes),
                        bytes_to_mb(model_file.size)
                    ),
                )?;
                last_logged_bytes = downloaded_bytes;
                last_log_at = Instant::now();
            }
        }
        target.flush().map_err(|err| err.to_string())?;
        target.sync_all().map_err(|err| err.to_string())?;
        Ok(())
    })();
    drop(target);
    if let Err(error) = download_result {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    if downloaded_bytes != model_file.size {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "模型文件下载不完整：{}，期望 {} 字节，实际 {} 字节。",
            model_file.path, model_file.size, downloaded_bytes
        ));
    }
    verify_bundled_asset_sha256(&temp_path, &model_file.sha256, log_path)?;
    fs::rename(&temp_path, target_path).map_err(|err| err.to_string())
}

fn validate_acquired_models(
    app: &AppHandle,
    python_executable: &Path,
    models_root: &Path,
    asr_backend: &str,
    log_path: &Path,
) -> LocalResult<()> {
    let generation_root = models_root
        .parent()
        .ok_or_else(|| "模型目录缺少 generation 根目录。".to_string())?;
    let manifest = load_manifest()?;
    let acquired_set = read_acquired_model_set(generation_root)
        .ok_or_else(|| "模型获取 marker 缺失或无效。".to_string())?;
    validate_acquired_model_set(generation_root, &manifest, &acquired_set, Some(log_path))?;

    let model_paths = DEFAULT_FUNASR_MODELS
        .iter()
        .map(|(role, _, _)| {
            let model = acquired_set
                .models
                .iter()
                .find(|model| model.role == *role)
                .ok_or_else(|| format!("模型获取 marker 缺少角色：{role}"))?;
            Ok((*role, models_root.join(&model.relative_path)))
        })
        .collect::<LocalResult<Vec<_>>>()?;
    let warmup_path = resolve_script_resource_path(app, "runtime_warmup.py")?;
    let mut command = Command::new(python_executable);
    command
        .env("PYTHONUTF8", "1")
        .env("LIBERTY_ASR_BACKEND", asr_backend)
        .env("FUNASR_PROFILE", &manifest.model_profile)
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
    for (role, path) in model_paths {
        let variable = match role {
            "model" => "FUNASR_MODEL",
            "vad" => "FUNASR_VAD_MODEL",
            "punc" => "FUNASR_PUNC_MODEL",
            "spk" => "FUNASR_SPK_MODEL",
            _ => return Err(format!("不支持的模型角色：{role}")),
        };
        command.env(variable, path);
    }
    run_command_with_log_timeout(
        &mut command,
        log_path,
        "Validating acquired ASR models offline",
        Duration::from_secs(10 * 60),
    )?;

    let mut publish_command = Command::new(python_executable);
    publish_command
        .env("PYTHONUTF8", "1")
        .arg("-c")
        .arg(MODELSCOPE_CACHE_METADATA_SCRIPT)
        .arg(generation_root.join(MODELS_ACQUIRED_MARKER))
        .arg(models_root);
    run_command_with_log_timeout(
        &mut publish_command,
        log_path,
        "Publishing ModelScope cache metadata",
        Duration::from_secs(60),
    )
}

fn validate_acquired_model_set(
    generation_root: &Path,
    manifest: &RuntimeManifest,
    acquired_set: &AcquiredModelSet,
    checksum_log_path: Option<&Path>,
) -> LocalResult<()> {
    validate_acquired_model_metadata(manifest, acquired_set)?;
    let models_root = generation_root.join("models");
    for model in &acquired_set.models {
        let model_root = models_root.join(safe_model_relative_path(&model.relative_path)?);
        if !model_root.is_dir() {
            return Err(format!("模型目录不存在：{}", model_root.display()));
        }
        for file in &model.files {
            let relative_file_path = safe_model_relative_path(&file.path)?;
            let path = model_root.join(relative_file_path);
            verify_model_file_size(&path, file.size)?;
            if let Some(log_path) = checksum_log_path {
                verify_bundled_asset_sha256(&path, &file.sha256, log_path)?;
            }
        }
    }
    Ok(())
}

fn validate_acquired_model_metadata(
    manifest: &RuntimeManifest,
    acquired_set: &AcquiredModelSet,
) -> LocalResult<()> {
    if acquired_set.model_set_version != manifest.model_set_version
        || acquired_set.model_profile != manifest.model_profile
    {
        return Err("模型获取 marker 与当前 manifest 不匹配。".into());
    }
    if acquired_set.models.len() != DEFAULT_FUNASR_MODELS.len() {
        return Err("模型获取 marker 未包含完整的默认模型集合。".into());
    }

    let mut roles = HashSet::new();
    for model in &acquired_set.models {
        if !roles.insert(model.role.as_str()) {
            return Err(format!("模型获取 marker 包含重复角色：{}", model.role));
        }
        let (expected_model_id, expected_revision) = DEFAULT_FUNASR_MODELS
            .iter()
            .find_map(|(role, model_id, revision)| {
                (*role == model.role).then_some((*model_id, *revision))
            })
            .ok_or_else(|| format!("模型获取 marker 包含未知角色：{}", model.role))?;
        if model.model_id != expected_model_id
            || model.revision != expected_revision
            || model.files.is_empty()
            || !model.files.iter().any(|file| file.size > 0)
        {
            return Err(format!(
                "模型获取 marker 的 {} 模型信息不完整。",
                model.role
            ));
        }
        let relative_model_path = safe_model_relative_path(&model.relative_path)?;
        let expected_model_path = model_cache_relative_path(&model.model_id)?;
        if relative_model_path != expected_model_path {
            return Err(format!("模型获取 marker 的 {} 目录不匹配。", model.role));
        }

        let mut file_paths = HashSet::new();
        for file in &model.files {
            let relative_file_path = safe_model_relative_path(&file.path)?;
            let normalized_path = path_to_forward_slashes(&relative_file_path)?;
            if !file_paths.insert(normalized_path.clone()) {
                return Err(format!("模型 marker 包含重复文件：{normalized_path}"));
            }
            if validate_model_revision(&file.revision).is_err() || !valid_sha256(&file.sha256) {
                return Err(format!("模型 marker 文件信息无效：{normalized_path}"));
            }
        }
    }
    Ok(())
}

fn model_cache_relative_path(model_id: &str) -> LocalResult<PathBuf> {
    validate_model_id(model_id)?;
    Ok(PathBuf::from("modelscope").join("models").join(model_id))
}

fn validate_model_id(model_id: &str) -> LocalResult<()> {
    let parts = model_id.split('/').collect::<Vec<_>>();
    if parts.len() != 2
        || parts.iter().any(|part| {
            part.is_empty()
                || matches!(*part, "." | "..")
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
    {
        return Err(format!("无效的 ModelScope 模型 ID：{model_id}"));
    }
    Ok(())
}

fn validate_model_revision(revision: &str) -> LocalResult<()> {
    if revision.len() == 40
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err("ModelScope 模型 revision 必须是固定的 40 位提交哈希。".into())
    }
}

fn safe_model_relative_path(value: &str) -> LocalResult<PathBuf> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || value.contains('\\')
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!("模型文件路径不安全：{value}"));
    }
    Ok(path.to_path_buf())
}

fn path_to_forward_slashes(path: &Path) -> LocalResult<String> {
    let parts = path
        .components()
        .map(|component| match component {
            std::path::Component::Normal(value) => value
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| "模型路径包含无效字符。".to_string()),
            _ => Err("模型路径不是安全相对路径。".to_string()),
        })
        .collect::<LocalResult<Vec<_>>>()?;
    Ok(parts.join("/"))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn verify_model_file_size(path: &Path, expected_size: u64) -> LocalResult<()> {
    if !path.is_file() || path.metadata().map_err(|err| err.to_string())?.len() != expected_size {
        return Err(format!("模型文件大小不匹配：{}", path.display()));
    }
    Ok(())
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> LocalResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| "marker 缺少父目录。".to_string())?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    let temp_path = path.with_extension("tmp");
    let mut file = File::create(&temp_path).map_err(|err| err.to_string())?;
    let payload = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    file.write_all(&payload).map_err(|err| err.to_string())?;
    file.flush().map_err(|err| err.to_string())?;
    file.sync_all().map_err(|err| err.to_string())?;
    drop(file);
    if path.exists() {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("marker 已存在，拒绝覆盖：{}", path.display()));
    }
    fs::rename(&temp_path, path).map_err(|err| err.to_string())
}

fn bytes_to_mb(value: u64) -> String {
    format!("{:.1}", value as f64 / 1024.0 / 1024.0)
}

fn persist_models_waiting_for_python(app: &AppHandle, generation: u64) -> LocalResult<()> {
    let platform_id = current_platform_id()?;
    let mut state =
        local_db::get_runtime_component_state(app, platform_id, COMPONENT_MODEL, SOURCE_MANAGED)?;
    if state.operation.generation != generation {
        return Ok(());
    }
    state.operation.kind = "waiting_for_python".into();
    state.operation.phase = "waiting_for_python".into();
    state.operation.progress = Some(100);
    state.operation.last_error = None;
    state.updated_at = unix_timestamp_millis().to_string();
    local_db::save_runtime_component_state(app, platform_id, &state)
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
        install_python_dependencies(
            app,
            &python_executable,
            download_source,
            platform_id,
            backend,
            log_path,
        )?;
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

    install_python_dependencies(
        app,
        &python_executable,
        download_source,
        platform_id,
        backend,
        log_path,
    )?;
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
    platform_id: &str,
    backend: &str,
    log_path: &Path,
) -> LocalResult<()> {
    let lock_file = match platform_id {
        "darwin-aarch64" => "requirements-lock-darwin-aarch64.txt",
        "darwin-x64" => "requirements-lock-darwin-x64.txt",
        "windows-x64" => "requirements-lock-windows-x64.txt",
        _ => {
            return Err(format!(
                "当前平台没有经过验证的 Python 依赖锁：{platform_id}"
            ))
        }
    };
    let requirements_path = resolve_script_resource_path(app, lock_file)?;
    let mut command = Command::new(python_executable);
    command
        .env("PYTHONUTF8", "1")
        .env("LIBERTY_ASR_BACKEND", backend)
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--disable-pip-version-check")
        .arg("--no-input")
        .arg("--require-hashes")
        .arg("--no-build-isolation");
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
    use super::{
        aggregate_runtime_status, load_manifest, model_cache_relative_path,
        model_can_start_with_python, model_install_action, path_to_forward_slashes,
        runtime_install_components, safe_model_relative_path, write_json_atomically, AcquiredModel,
        AcquiredModelFile, AcquiredModelSet, ModelInstallAction, COMPONENT_FFMPEG, COMPONENT_MODEL,
        COMPONENT_PYTHON, DEFAULT_FUNASR_MODELS, MODELS_ACQUIRED_MARKER,
    };
    use crate::local_db::{
        AppSettings, ManagedRuntimeState, RuntimeArtifactState, RuntimeComponentState,
        RuntimeOperationState,
    };
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn acquired_models_fixture() -> (PathBuf, AcquiredModelSet) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let generation_root = std::env::temp_dir().join(format!(
            "liberty-acquired-models-{}-{unique}",
            std::process::id()
        ));
        let models_root = generation_root.join("models");
        let mut models = Vec::new();
        for (role, model_id, revision) in DEFAULT_FUNASR_MODELS {
            let relative_path = model_cache_relative_path(model_id).expect("model path");
            let model_root = models_root.join(&relative_path);
            fs::create_dir_all(&model_root).expect("model root");
            let contents = role.as_bytes();
            fs::write(model_root.join("config.yaml"), contents).expect("model file");
            models.push(AcquiredModel {
                role: role.into(),
                model_id: model_id.into(),
                revision: revision.into(),
                relative_path: path_to_forward_slashes(&relative_path).expect("relative path"),
                files: vec![AcquiredModelFile {
                    path: "config.yaml".into(),
                    revision: revision.into(),
                    sha256: "a".repeat(64),
                    size: contents.len() as u64,
                }],
            });
        }
        let manifest = load_manifest().expect("runtime manifest");
        let acquired_set = AcquiredModelSet {
            model_set_version: manifest.model_set_version,
            model_profile: manifest.model_profile,
            models,
        };
        write_json_atomically(&generation_root.join(MODELS_ACQUIRED_MARKER), &acquired_set)
            .expect("acquired marker");
        (generation_root, acquired_set)
    }

    #[test]
    fn batch_install_starts_models_before_ffmpeg() {
        let settings = AppSettings {
            python_runtime_source: "managed".into(),
            ffmpeg_runtime_source: "managed".into(),
            ..AppSettings::default()
        };

        assert_eq!(
            runtime_install_components(&settings),
            [COMPONENT_PYTHON, COMPONENT_MODEL, COMPONENT_FFMPEG]
        );
    }

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
    fn acquired_models_wait_then_validate_without_reacquiring() {
        let (generation_root, _) = acquired_models_fixture();
        let manifest = load_manifest().expect("runtime manifest");
        let unavailable_python = RuntimeComponentState::unavailable("python", Some("managed"));

        assert_eq!(
            model_install_action(&generation_root, &manifest, &unavailable_python),
            ModelInstallAction::WaitForPython
        );

        let mut ready_python = unavailable_python;
        ready_python.availability = "ready".into();
        ready_python.active_artifact = Some(RuntimeArtifactState {
            generation_id: "1".into(),
            artifact_version: "runtime".into(),
            resolved_path: "/runtime/python".into(),
        });
        assert_eq!(
            model_install_action(&generation_root, &manifest, &ready_python),
            ModelInstallAction::Validate
        );

        fs::remove_dir_all(generation_root).expect("remove fixture");
    }

    #[test]
    fn incomplete_acquired_marker_requires_acquisition() {
        let (generation_root, acquired_set) = acquired_models_fixture();
        let manifest = load_manifest().expect("runtime manifest");
        let python = RuntimeComponentState::unavailable("python", Some("managed"));
        let missing_file = generation_root
            .join("models")
            .join(&acquired_set.models[0].relative_path)
            .join(&acquired_set.models[0].files[0].path);
        fs::remove_file(missing_file).expect("remove model file");

        assert_eq!(
            model_install_action(&generation_root, &manifest, &python),
            ModelInstallAction::Acquire
        );

        fs::remove_dir_all(generation_root).expect("remove fixture");
    }

    #[test]
    fn model_paths_reject_escape_components() {
        for path in ["../model", "/model", "model\\file", "model//file"] {
            assert!(safe_model_relative_path(path).is_err(), "accepted {path}");
        }
        assert!(safe_model_relative_path("model/config.yaml").is_ok());
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
