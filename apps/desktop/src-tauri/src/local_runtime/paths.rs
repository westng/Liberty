use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager};

use crate::local_db::LocalResult;
use crate::local_runtime::manifest::{current_platform_manifest, load_manifest, PlatformRuntime};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn resolve_python_executable(
    runtime_root: &Path,
    platform: &PlatformRuntime,
) -> LocalResult<PathBuf> {
    for candidate in &platform.python_executable_candidates {
        let path = runtime_root.join(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }

    Err("未找到托管运行环境中的 Python 可执行文件。".into())
}

pub fn find_python_executable(root: &Path) -> Option<PathBuf> {
    let candidates = if cfg!(windows) {
        ["python.exe", "python3.exe"]
    } else {
        ["python3", "python"]
    };
    find_named_executable(root, &candidates)
}

pub fn resolve_ffmpeg_executable(
    runtime_root: &Path,
    platform: &PlatformRuntime,
) -> Option<PathBuf> {
    for candidate in &platform.ffmpeg_executable_candidates {
        let path = runtime_root.join(candidate);
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

pub fn find_ffmpeg_executable(root: &Path) -> Option<PathBuf> {
    let candidates = if cfg!(windows) {
        ["ffmpeg.exe", "ffmpeg.exe"]
    } else {
        ["ffmpeg", "ffmpeg"]
    };
    find_named_executable(root, &candidates)
}

fn find_named_executable(root: &Path, names: &[&str; 2]) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if names
                .iter()
                .any(|candidate| name.eq_ignore_ascii_case(candidate))
            {
                return Some(path);
            }
        }
    }

    None
}

pub fn resolve_managed_ffmpeg_path(runtime_root: &Path) -> LocalResult<Option<PathBuf>> {
    let manifest = load_manifest()?;
    let platform = current_platform_manifest(&manifest)?;
    let ffmpeg_path = resolve_ffmpeg_executable(runtime_root, &platform);
    if let Some(path) = ffmpeg_path.as_ref() {
        ensure_unix_executable(path)?;
    }
    Ok(ffmpeg_path)
}

pub fn ensure_unix_executable(path: &Path) -> LocalResult<()> {
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

pub fn resolve_script_resource_path(app: &AppHandle, file_name: &str) -> LocalResult<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join(file_name));
        candidates.push(resource_dir.join(file_name));
        candidates.push(resource_dir.join("_up_").join("scripts").join(file_name));
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(
        manifest_dir
            .join("../../../python/funasr-runner")
            .join(file_name),
    );

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(
            current_dir
                .join("../../python/funasr-runner")
                .join(file_name),
        );
        candidates.push(current_dir.join("python/funasr-runner").join(file_name));
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
