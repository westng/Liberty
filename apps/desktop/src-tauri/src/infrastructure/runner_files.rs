use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::local_db::LocalResult;

pub fn validate_job_id(job_id: &str) -> LocalResult<()> {
    if job_id.len() > 64 || job_id.trim() != job_id {
        return Err("任务 ID 格式无效。".into());
    }

    let mut parts = job_id.split('-');
    let prefix = parts.next();
    let timestamp = parts.next();
    let sequence = parts.next();
    if prefix != Some("job")
        || parts.next().is_some()
        || !timestamp.is_some_and(|value| {
            value.len() == 13 && value.bytes().all(|byte| byte.is_ascii_digit())
        })
        || !sequence.is_some_and(|value| {
            !value.is_empty()
                && value.len() <= 20
                && value.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err("任务 ID 格式无效。".into());
    }

    Ok(())
}

pub fn resolve_job_dir(jobs_root: &Path, job_id: &str) -> LocalResult<PathBuf> {
    validate_job_id(job_id)?;
    let canonical_root = canonical_directory(jobs_root, "任务根目录")?;
    let candidate = canonical_root.join(job_id);

    match fs::symlink_metadata(&candidate) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(format!("任务目录不能是符号链接: {}", candidate.display()))
        }
        Ok(metadata) if !metadata.is_dir() => {
            Err(format!("任务路径不是目录: {}", candidate.display()))
        }
        Ok(_) => {
            let canonical_candidate = candidate
                .canonicalize()
                .map_err(|err| format!("无法规范化任务目录 {}: {err}", candidate.display()))?;
            ensure_contained(&canonical_root, &canonical_candidate)?;
            Ok(canonical_candidate)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            ensure_contained(&canonical_root, &candidate)?;
            Ok(candidate)
        }
        Err(error) => Err(format!("无法检查任务目录 {}: {error}", candidate.display())),
    }
}

pub fn create_attempt_dir(
    job_dir: &Path,
    attempt_id: u64,
    lease_token: u64,
) -> LocalResult<PathBuf> {
    resolve_attempt_dir_inner(job_dir, attempt_id, lease_token, true)?
        .ok_or_else(|| "无法创建任务 attempt 目录。".to_string())
}

pub fn resolve_attempt_dir(
    job_dir: &Path,
    attempt_id: u64,
    lease_token: u64,
) -> LocalResult<Option<PathBuf>> {
    resolve_attempt_dir_inner(job_dir, attempt_id, lease_token, false)
}

fn resolve_attempt_dir_inner(
    job_dir: &Path,
    attempt_id: u64,
    lease_token: u64,
    create: bool,
) -> LocalResult<Option<PathBuf>> {
    if attempt_id == 0 || lease_token == 0 {
        return Err("任务 attempt 或 lease 无效。".into());
    }

    let canonical_job_dir = canonical_directory(job_dir, "任务目录")?;
    let attempts_candidate = canonical_job_dir.join("attempts");
    let Some(attempts_root) = resolve_child_directory(
        &canonical_job_dir,
        &attempts_candidate,
        "任务 attempts 目录",
        create,
    )?
    else {
        return Ok(None);
    };
    let attempt_candidate = attempts_root.join(format!("attempt-{attempt_id}-{lease_token}"));
    resolve_child_directory(
        &attempts_root,
        &attempt_candidate,
        "任务 attempt 目录",
        create,
    )
}

fn resolve_child_directory(
    root: &Path,
    candidate: &Path,
    label: &str,
    create: bool,
) -> LocalResult<Option<PathBuf>> {
    let metadata = match fs::symlink_metadata(candidate) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound && !create => return Ok(None),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            match fs::create_dir(candidate) {
                Ok(()) => {}
                Err(create_error) if create_error.kind() == ErrorKind::AlreadyExists => {}
                Err(create_error) => {
                    return Err(format!(
                        "无法创建{label} {}: {create_error}",
                        candidate.display()
                    ));
                }
            }
            fs::symlink_metadata(candidate).map_err(|metadata_error| {
                format!("无法检查{label} {}: {metadata_error}", candidate.display())
            })?
        }
        Err(error) => return Err(format!("无法检查{label} {}: {error}", candidate.display())),
    };

    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label}不是可信目录: {}", candidate.display()));
    }
    let canonical_candidate = canonical_directory(candidate, label)?;
    ensure_contained(root, &canonical_candidate)?;
    Ok(Some(canonical_candidate))
}

pub fn move_job_dir_to_trash(jobs_root: &Path, job_id: &str, trash_name: &str) -> LocalResult<()> {
    validate_trash_name(trash_name)?;
    let original_path = resolve_job_dir(jobs_root, job_id)?;
    let canonical_root = canonical_directory(jobs_root, "任务根目录")?;
    let trash_root = resolve_trash_root(&canonical_root, true)?
        .ok_or_else(|| "无法创建任务回收目录。".to_string())?;
    let trash_path = trash_root.join(trash_name);
    ensure_contained(&trash_root, &trash_path)?;

    let original_exists = trusted_directory_exists(&original_path, "任务目录")?;
    let trash_exists = trusted_directory_exists(&trash_path, "任务回收项")?;
    match (original_exists, trash_exists) {
        (false, _) => return Ok(()),
        (true, true) => {
            return Err(format!(
                "任务目录和回收目录同时存在，拒绝覆盖: {} / {}",
                original_path.display(),
                trash_path.display()
            ));
        }
        (true, false) => {}
    }

    fs::rename(&original_path, &trash_path).map_err(|err| {
        format!(
            "无法把任务目录移入回收区 {} -> {}: {err}",
            original_path.display(),
            trash_path.display()
        )
    })?;
    Ok(())
}

pub fn purge_job_trash(jobs_root: &Path, trash_name: &str) -> LocalResult<()> {
    validate_trash_name(trash_name)?;
    let canonical_root = canonical_directory(jobs_root, "任务根目录")?;
    let Some(trash_root) = resolve_trash_root(&canonical_root, false)? else {
        return Ok(());
    };
    let trash_path = trash_root.join(trash_name);
    ensure_contained(&trash_root, &trash_path)?;
    if !trusted_directory_exists(&trash_path, "任务回收项")? {
        return Ok(());
    }
    fs::remove_dir_all(&trash_path).map_err(|err| {
        format!(
            "任务记录已删除，但回收目录清理失败 {}: {err}",
            trash_path.display()
        )
    })
}

pub fn reset_runner_files(job_dir: &Path) -> LocalResult<()> {
    for name in ["result.json", "progress.json", "job.json"] {
        let path = job_dir.join(name);
        if path.exists() {
            fs::remove_file(path).map_err(|err| err.to_string())?;
        }
    }

    Ok(())
}

fn canonical_directory(path: &Path, label: &str) -> LocalResult<PathBuf> {
    path.canonicalize()
        .map_err(|err| format!("无法规范化{label} {}: {err}", path.display()))
}

fn resolve_trash_root(canonical_jobs_root: &Path, create: bool) -> LocalResult<Option<PathBuf>> {
    resolve_child_directory(
        canonical_jobs_root,
        &canonical_jobs_root.join(".trash"),
        "任务回收目录",
        create,
    )
}

fn trusted_directory_exists(path: &Path, label: &str) -> LocalResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(format!("{label}不是可信目录: {}", path.display()))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("无法检查{label} {}: {error}", path.display())),
    }
}

fn validate_trash_name(trash_name: &str) -> LocalResult<()> {
    if trash_name.is_empty()
        || trash_name.len() > 192
        || !trash_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("任务回收目录名称无效。".into());
    }
    Ok(())
}

fn ensure_contained(root: &Path, candidate: &Path) -> LocalResult<()> {
    if candidate.starts_with(root) && candidate != root {
        Ok(())
    } else {
        Err(format!("任务路径越过允许的根目录: {}", candidate.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "liberty-runner-files-{name}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("temp jobs root");
        root
    }

    #[test]
    fn accepts_only_backend_job_id_format() {
        assert!(validate_job_id("job-1700000000000-0").is_ok());
        for invalid in [
            "../job-1700000000000-0",
            "job-1700000000000-0/child",
            "job-1-0",
            "job-1700000000000-x",
            " job-1700000000000-0",
        ] {
            assert!(validate_job_id(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn resolves_missing_job_only_below_canonical_root() {
        let root = temp_root("resolve");
        let resolved = resolve_job_dir(&root, "job-1700000000000-1").unwrap();
        assert_eq!(
            resolved.parent(),
            Some(root.canonicalize().unwrap().as_path())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_job_directory_symlink_that_escapes_root() {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink");
        let outside = temp_root("outside");
        symlink(&outside, root.join("job-1700000000000-2")).unwrap();
        assert!(resolve_job_dir(&root, "job-1700000000000-2").is_err());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_job_directory_symlink_to_sibling_job() {
        use std::os::unix::fs::symlink;

        let root = temp_root("sibling-symlink");
        let sibling = root.join("job-1700000000000-7");
        fs::create_dir(&sibling).unwrap();
        symlink(&sibling, root.join("job-1700000000000-8")).unwrap();

        assert!(resolve_job_dir(&root, "job-1700000000000-8").is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn trash_move_and_purge_are_idempotent() {
        let root = temp_root("trash");
        let first_id = "job-1700000000000-3";
        let first_dir = root.join(first_id);
        fs::create_dir_all(&first_dir).unwrap();
        fs::write(first_dir.join("result.json"), b"{}").unwrap();
        let trash_name = "job-1700000000000-3-delete-1700000000000-1";

        move_job_dir_to_trash(&root, first_id, trash_name).unwrap();
        move_job_dir_to_trash(&root, first_id, trash_name).unwrap();
        assert!(!first_dir.exists());
        let trash_path = root.join(".trash").join(trash_name);
        assert!(trash_path.exists());
        purge_job_trash(&root, trash_name).unwrap();
        purge_job_trash(&root, trash_name).unwrap();
        assert!(!trash_path.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_attempt_directory_inside_job_directory() {
        let root = temp_root("attempt-directory");
        let job_dir = root.join("job-1700000000000-4");
        fs::create_dir(&job_dir).unwrap();

        let created = create_attempt_dir(&job_dir, 2, 7).unwrap();
        let resolved = resolve_attempt_dir(&job_dir, 2, 7).unwrap();

        assert_eq!(resolved.as_deref(), Some(created.as_path()));
        assert!(created.ends_with("attempts/attempt-2-7"));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_attempts_directory() {
        use std::os::unix::fs::symlink;

        let root = temp_root("attempt-symlink");
        let outside = temp_root("attempt-outside");
        let job_dir = root.join("job-1700000000000-5");
        fs::create_dir(&job_dir).unwrap();
        symlink(&outside, job_dir.join("attempts")).unwrap();

        assert!(create_attempt_dir(&job_dir, 1, 1).is_err());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }
}
