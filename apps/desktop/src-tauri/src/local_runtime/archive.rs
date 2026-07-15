use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use sevenz_rust2::decompress_file;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};
use zip::ZipArchive;

use crate::local_db::LocalResult;
use crate::local_runtime::logging::append_install_log_line;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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

    if target_path.is_file() && target_path.metadata().map(|meta| meta.len()).unwrap_or(0) > 0 {
        append_install_log_line(
            log_path,
            &format!(
                "[runtime] reusing downloaded asset {}",
                target_path.display()
            ),
        )?;
        return Ok(());
    }

    let temp_path = target_path.with_extension("download");
    let _ = fs::remove_file(&temp_path);
    let _ = fs::remove_file(target_path);

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
    if expected.trim().is_empty() {
        return Ok(());
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

    if file_name.ends_with(".7z") {
        return extract_7z(archive_path, destination, log_path, description);
    }

    if file_name.ends_with(".gz") {
        return extract_gzip_file(archive_path, destination, log_path, description);
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

fn extract_7z(
    archive_path: &Path,
    destination: &Path,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    append_install_log_line(log_path, &format!("[runtime] {description}"))?;
    fs::create_dir_all(destination).map_err(|err| err.to_string())?;
    decompress_file(archive_path, destination).map_err(|err| err.to_string())
}

fn extract_gzip_file(
    archive_path: &Path,
    destination: &Path,
    log_path: &Path,
    description: &str,
) -> LocalResult<()> {
    append_install_log_line(log_path, &format!("[runtime] {description}"))?;
    fs::create_dir_all(destination).map_err(|err| err.to_string())?;
    let output_name = archive_path
        .file_stem()
        .ok_or_else(|| format!("无法推断压缩包输出文件名：{}", archive_path.display()))?;
    let output_path = destination.join(output_name);
    let input = File::open(archive_path).map_err(|err| err.to_string())?;
    let mut decoder = GzDecoder::new(input);
    let mut output = File::create(&output_path).map_err(|err| err.to_string())?;
    std::io::copy(&mut decoder, &mut output).map_err(|err| err.to_string())?;
    output.flush().map_err(|err| err.to_string())?;
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
    let archive_file = File::open(archive_path).map_err(|err| err.to_string())?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_mtime(false);
    archive.unpack(destination).map_err(|err| err.to_string())
}
