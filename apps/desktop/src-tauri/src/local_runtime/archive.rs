use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};
use zip::ZipArchive;

use crate::local_db::LocalResult;
use crate::local_runtime::logging::append_install_log_line;
use crate::local_runtime::manifest::expected_asset_sha256;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const MAX_ARCHIVE_ENTRIES: usize = 100_000;
const MAX_EXPANDED_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const MAX_SINGLE_ENTRY_BYTES: u64 = 8 * 1024 * 1024 * 1024;

pub fn download_remote_asset(
    download_url: &str,
    target_path: &Path,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    let trimmed_url = download_url.trim();
    if trimmed_url.is_empty() {
        return Err(format!("{description} 缺少下载地址。"));
    }

    append_install_log_line(log_path, &format!("[runtime] {description}"))?;
    append_install_log_line(log_path, &format!("[runtime] download url {trimmed_url}"))?;

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("{description} 缺少有效文件名。"))?;
    let expected_sha256 = expected_asset_sha256(file_name, trimmed_url)?;

    if target_path.is_file() && target_path.metadata().map(|meta| meta.len()).unwrap_or(0) > 0 {
        match verify_bundled_asset_sha256(target_path, &expected_sha256, log_path) {
            Ok(()) => {
                append_install_log_line(
                    log_path,
                    &format!("[runtime] reusing verified asset {}", target_path.display()),
                )?;
                return Ok(());
            }
            Err(error) => append_install_log_line(
                log_path,
                &format!("[runtime] cached asset rejected: {error}"),
            )?,
        }
    }

    let temp_path = target_path.with_extension("download");
    let _ = fs::remove_file(&temp_path);

    let client = Client::builder()
        .timeout(Duration::from_secs(60 * 30))
        .build()
        .map_err(|err| err.to_string())?;
    let mut response = client
        .get(trimmed_url)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?;
    let total_bytes = response.content_length().unwrap_or(0);
    if total_bytes > 0 {
        append_install_log_line(
            log_path,
            &format!(
                "[runtime] remote asset size {} MB",
                bytes_to_mb(total_bytes)
            ),
        )?;
    }

    let mut target = File::create(&temp_path).map_err(|err| err.to_string())?;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut downloaded_bytes: u64 = 0;
    let mut last_logged_bytes: u64 = 0;
    let mut last_log_at = Instant::now();

    loop {
        let read = response.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }

        target
            .write_all(&buffer[..read])
            .map_err(|err| err.to_string())?;
        downloaded_bytes += read as u64;

        let should_log = downloaded_bytes.saturating_sub(last_logged_bytes) >= 64 * 1024 * 1024
            || last_log_at.elapsed() >= Duration::from_secs(5);
        if should_log {
            if total_bytes > 0 {
                append_install_log_line(
                    log_path,
                    &format!(
                        "[runtime] download progress {} / {} MB ({:.1}%)",
                        bytes_to_mb(downloaded_bytes),
                        bytes_to_mb(total_bytes),
                        downloaded_bytes as f64 / total_bytes as f64 * 100.0
                    ),
                )?;
            } else {
                append_install_log_line(
                    log_path,
                    &format!(
                        "[runtime] download progress {} MB",
                        bytes_to_mb(downloaded_bytes)
                    ),
                )?;
            }
            last_logged_bytes = downloaded_bytes;
            last_log_at = Instant::now();
        }
    }

    if total_bytes > 0 && downloaded_bytes != total_bytes {
        return Err(format!(
            "{description} 下载不完整，期望 {} 字节，实际 {} 字节。",
            total_bytes, downloaded_bytes
        ));
    }

    target.flush().map_err(|err| err.to_string())?;
    target.sync_all().map_err(|err| err.to_string())?;
    drop(target);
    verify_bundled_asset_sha256(&temp_path, &expected_sha256, log_path)?;
    let _ = fs::remove_file(target_path);
    fs::rename(&temp_path, target_path).map_err(|err| err.to_string())
}

fn bytes_to_mb(value: u64) -> String {
    format!("{:.1}", value as f64 / 1024.0 / 1024.0)
}

pub fn verify_bundled_asset_sha256(
    path: &Path,
    expected: &str,
    log_path: &Path,
) -> LocalResult<()> {
    let expected = expected.trim();
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("运行时资源缺少有效的 SHA-256，拒绝安装。".into());
    }

    append_install_log_line(log_path, "[runtime] verifying runtime asset checksum")?;
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

    let _ = fs::remove_file(path);
    Err(format!(
        "运行时资源校验失败，期望 {expected}，实际 {digest}。"
    ))
}

pub fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    let file_name = archive_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    if file_name.ends_with(".tar.gz") {
        return extract_tar_gz(archive_path, destination, log_path, description);
    }

    if file_name.ends_with(".zip") {
        return extract_zip(archive_path, destination, log_path, description);
    }

    if file_name.ends_with(".7z") || file_name.ends_with(".gz") {
        return Err(format!(
            "运行时压缩包格式缺少安全预检，拒绝解压：{}",
            archive_path.display()
        ));
    }

    Err(format!(
        "不支持的运行时压缩包格式：{}",
        archive_path.display()
    ))
}

fn extract_zip(
    archive_path: &Path,
    destination: &Path,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    append_install_log_line(log_path, &format!("[runtime] {description}"))?;
    preflight_zip(archive_path)?;
    prepare_destination(destination)?;
    let archive_file = File::open(archive_path).map_err(|err| err.to_string())?;
    let mut archive = ZipArchive::new(archive_file).map_err(|err| err.to_string())?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|err| err.to_string())?;
        let entry_name = safe_archive_path(entry.name())?;
        let output_path = destination.join(entry_name);

        if entry.is_dir() {
            create_safe_directories(destination, output_path.strip_prefix(destination).unwrap())?;
            continue;
        }

        let parent = output_path
            .parent()
            .and_then(|value| value.strip_prefix(destination).ok())
            .unwrap_or(Path::new(""));
        create_safe_directories(destination, parent)?;

        let mut output = File::create(&output_path).map_err(|err| err.to_string())?;
        std::io::copy(&mut entry, &mut output).map_err(|err| err.to_string())?;
        output.flush().map_err(|err| err.to_string())?;
        apply_zip_entry_permissions(&output_path, entry.unix_mode())?;
    }

    Ok(())
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

fn extract_tar_gz(
    archive_path: &Path,
    destination: &Path,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    append_install_log_line(log_path, &format!("[runtime] {description}"))?;
    preflight_tar_gz(archive_path)?;
    prepare_destination(destination)?;

    extract_tar_pass(archive_path, destination, false)?;
    extract_tar_pass(archive_path, destination, true)
}

fn preflight_zip(archive_path: &Path) -> LocalResult<()> {
    let archive_file = File::open(archive_path).map_err(|err| err.to_string())?;
    let mut archive = ZipArchive::new(archive_file).map_err(|err| err.to_string())?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(format!("ZIP 条目数超过限制：{}", archive.len()));
    }

    let mut expanded_bytes = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|err| err.to_string())?;
        safe_archive_path(entry.name())?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!("ZIP 包含不允许的符号链接：{}", entry.name()));
        }
        expanded_bytes = checked_expanded_size(expanded_bytes, entry.size(), entry.name())?;
    }
    Ok(())
}

fn preflight_tar_gz(archive_path: &Path) -> LocalResult<()> {
    let archive_file = File::open(archive_path).map_err(|err| err.to_string())?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_mtime(false);
    let entries = archive.entries().map_err(|err| err.to_string())?;
    let mut entry_count = 0usize;
    let mut expanded_bytes = 0u64;

    for entry in entries {
        let entry = entry.map_err(|err| err.to_string())?;
        entry_count += 1;
        if entry_count > MAX_ARCHIVE_ENTRIES {
            return Err(format!("TAR 条目数超过限制：{entry_count}"));
        }
        let entry_path = safe_path(entry.path().map_err(|err| err.to_string())?.as_ref())?;
        let entry_type = entry.header().entry_type();
        if !(entry_type.is_file()
            || entry_type.is_dir()
            || entry_type.is_symlink()
            || entry_type.is_hard_link())
        {
            return Err(format!(
                "TAR 包含不允许的条目类型：{}",
                entry_path.display()
            ));
        }
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            let target = entry
                .link_name()
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("TAR 链接缺少目标：{}", entry_path.display()))?;
            validate_tar_link_target(&entry_path, target.as_ref(), entry_type.is_hard_link())?;
        }
        expanded_bytes = checked_expanded_size(
            expanded_bytes,
            entry.size(),
            &entry_path.display().to_string(),
        )?;
    }
    Ok(())
}

fn extract_tar_pass(archive_path: &Path, destination: &Path, links_only: bool) -> LocalResult<()> {
    let archive_file = File::open(archive_path).map_err(|err| err.to_string())?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_mtime(false);
    for entry in archive.entries().map_err(|err| err.to_string())? {
        let mut entry = entry.map_err(|err| err.to_string())?;
        let entry_type = entry.header().entry_type();
        let is_link = entry_type.is_symlink() || entry_type.is_hard_link();
        if is_link != links_only {
            continue;
        }
        let relative_path = safe_path(entry.path().map_err(|err| err.to_string())?.as_ref())?;
        let parent = relative_path.parent().unwrap_or(Path::new(""));
        create_safe_directories(destination, parent)?;
        if !entry
            .unpack_in(destination)
            .map_err(|err| err.to_string())?
        {
            return Err(format!("TAR 条目逃逸解压目录：{}", relative_path.display()));
        }
    }
    Ok(())
}

fn checked_expanded_size(current: u64, size: u64, name: &str) -> LocalResult<u64> {
    if size > MAX_SINGLE_ENTRY_BYTES {
        return Err(format!("压缩包单个条目超过限制：{name}"));
    }
    let total = current
        .checked_add(size)
        .ok_or_else(|| "压缩包展开大小溢出。".to_string())?;
    if total > MAX_EXPANDED_BYTES {
        return Err("压缩包展开大小超过 20 GiB 限制。".into());
    }
    Ok(total)
}

fn safe_archive_path(value: &str) -> LocalResult<PathBuf> {
    if value.contains('\\') {
        return Err(format!("压缩包路径使用不允许的分隔符：{value}"));
    }
    safe_path(Path::new(value))
}

fn safe_path(path: &Path) -> LocalResult<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("压缩包路径试图逃逸目标目录：{}", path.display()));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("压缩包包含空路径条目。".into());
    }
    Ok(normalized)
}

fn validate_tar_link_target(entry: &Path, target: &Path, hard_link: bool) -> LocalResult<()> {
    let mut resolved = if hard_link {
        PathBuf::new()
    } else {
        entry.parent().unwrap_or(Path::new("")).to_path_buf()
    };
    for component in target.components() {
        match component {
            Component::Normal(value) => resolved.push(value),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return Err(format!("TAR 链接试图逃逸目标目录：{}", entry.display()));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("TAR 链接使用绝对目标：{}", entry.display()));
            }
        }
    }
    if resolved.as_os_str().is_empty() {
        return Err(format!("TAR 链接目标无效：{}", entry.display()));
    }
    Ok(())
}

fn prepare_destination(destination: &Path) -> LocalResult<()> {
    if let Ok(metadata) = fs::symlink_metadata(destination) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!("解压目标必须是真实目录：{}", destination.display()));
        }
    } else {
        fs::create_dir_all(destination).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn create_safe_directories(destination: &Path, relative: &Path) -> LocalResult<()> {
    let mut current = destination.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err(format!("无效的解压目录：{}", relative.display()));
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(format!(
                    "解压路径穿过非目录或符号链接：{}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|err| err.to_string())?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{extract_archive, verify_bundled_asset_sha256};
    use flate2::{write::GzEncoder, Compression};
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };
    use tar::{Builder, EntryType, Header};
    use zip::{write::SimpleFileOptions, ZipWriter};

    static TEST_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "liberty-archive-test-{}-{}",
                std::process::id(),
                TEST_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn checksum_is_required_and_mismatched_cache_is_removed() {
        let root = TestDir::new();
        let asset = root.path().join("asset.zip");
        let log = root.path().join("install.log");
        fs::write(&asset, b"untrusted cache").expect("write cached asset");

        assert!(verify_bundled_asset_sha256(&asset, "", &log).is_err());
        assert!(asset.exists());
        assert!(verify_bundled_asset_sha256(&asset, &"0".repeat(64), &log).is_err());
        assert!(!asset.exists());
    }

    #[test]
    fn valid_checksum_is_accepted() {
        let root = TestDir::new();
        let asset = root.path().join("asset.zip");
        let log = root.path().join("install.log");
        let content = b"verified asset";
        fs::write(&asset, content).expect("write asset");
        let expected = format!("{:x}", Sha256::digest(content));

        verify_bundled_asset_sha256(&asset, &expected, &log).expect("checksum should match");
    }

    #[test]
    fn archives_without_safe_preflight_are_rejected_before_writes() {
        for file_name in ["runtime.7z", "runtime.gz"] {
            let root = TestDir::new();
            let archive_path = root.path().join(file_name);
            let destination = root.path().join("output");
            let log = root.path().join("install.log");
            fs::write(&archive_path, b"unsupported archive").expect("write archive");

            let error = extract_archive(&archive_path, &destination, &log, "test")
                .expect_err("archive format should be rejected");

            assert!(error.contains("缺少安全预检，拒绝解压"));
            assert!(!destination.exists());
            assert!(!log.exists());
        }
    }

    #[test]
    fn zip_parent_traversal_is_rejected_before_writes() {
        let root = TestDir::new();
        let archive_path = root.path().join("malicious.zip");
        let destination = root.path().join("output");
        let log = root.path().join("install.log");
        let file = File::create(&archive_path).expect("create zip");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("../escape.txt", SimpleFileOptions::default())
            .expect("start zip entry");
        writer.write_all(b"escape").expect("write zip entry");
        writer.finish().expect("finish zip");

        assert!(extract_archive(&archive_path, &destination, &log, "test").is_err());
        assert!(!destination.exists());
        assert!(!root.path().join("escape.txt").exists());
    }

    #[test]
    fn tar_escaping_symlink_is_rejected_before_writes() {
        let root = TestDir::new();
        let archive_path = root.path().join("malicious.tar.gz");
        let destination = root.path().join("output");
        let log = root.path().join("install.log");
        let encoder = GzEncoder::new(
            File::create(&archive_path).expect("create tar.gz"),
            Compression::default(),
        );
        let mut builder = Builder::new(encoder);
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Symlink);
        header.set_mode(0o777);
        header.set_size(0);
        header.set_cksum();
        builder
            .append_link(&mut header, "dir/link", "../../escape")
            .expect("append symlink");
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip");

        assert!(extract_archive(&archive_path, &destination, &log, "test").is_err());
        assert!(!destination.exists());
    }
}
