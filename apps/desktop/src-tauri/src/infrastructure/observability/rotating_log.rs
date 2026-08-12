use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub const DEFAULT_FILE_LIMIT_BYTES: u64 = 512 * 1024;
pub const DEFAULT_FILE_COUNT: usize = 3;

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("log");
    path.with_file_name(format!("{name}.{index}"))
}

pub fn append(
    path: &Path,
    bytes: &[u8],
    file_limit_bytes: u64,
    file_count: usize,
) -> std::io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    let current_size = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if current_size > 0 && current_size.saturating_add(bytes.len() as u64) > file_limit_bytes {
        rotate(path, file_count)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(bytes)
}

fn rotate(path: &Path, file_count: usize) -> std::io::Result<()> {
    if file_count <= 1 {
        let _ = fs::remove_file(path);
        return Ok(());
    }
    let _ = fs::remove_file(rotated_path(path, file_count - 1));
    for index in (1..file_count - 1).rev() {
        let source = rotated_path(path, index);
        if source.exists() {
            fs::rename(source, rotated_path(path, index + 1))?;
        }
    }
    if path.exists() {
        fs::rename(path, rotated_path(path, 1))?;
    }
    Ok(())
}

pub fn reset(path: &Path, file_count: usize) -> std::io::Result<()> {
    fs::write(path, [])?;
    for index in 1..file_count {
        let _ = fs::remove_file(rotated_path(path, index));
    }
    Ok(())
}

pub fn read_recent(path: &Path, max_bytes: usize, file_count: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    let mut remaining = max_bytes;
    let mut chunks = Vec::new();
    for index in 0..file_count {
        let candidate = if index == 0 {
            path.to_path_buf()
        } else {
            rotated_path(path, index)
        };
        if remaining == 0 || !candidate.exists() {
            continue;
        }
        let bytes = read_tail(&candidate, remaining);
        remaining = remaining.saturating_sub(bytes.len());
        chunks.push(bytes);
    }
    chunks.reverse();
    let mut content = String::from_utf8_lossy(&chunks.concat()).into_owned();
    if content.len() >= max_bytes {
        if let Some(first_newline) = content.find('\n') {
            content.drain(..=first_newline);
        }
    }
    content.trim().to_string()
}

fn read_tail(path: &Path, max_bytes: usize) -> Vec<u8> {
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let Ok(file_size) = file.metadata().map(|metadata| metadata.len()) else {
        return Vec::new();
    };
    let read_size = file_size.min(max_bytes as u64);
    if file
        .seek(SeekFrom::Start(file_size.saturating_sub(read_size)))
        .is_err()
    {
        return Vec::new();
    }
    let mut bytes = vec![0; read_size as usize];
    if file.read_exact(&mut bytes).is_err() {
        return Vec::new();
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_enforces_total_limit_across_restart() {
        let root = std::env::temp_dir().join(format!(
            "liberty-log-rotation-{}-{}",
            std::process::id(),
            crate::infrastructure::time::unix_timestamp_millis()
        ));
        fs::create_dir_all(&root).expect("log directory");
        let path = root.join("diagnostic.log");
        for index in 0..8 {
            append(&path, format!("line-{index}-123456789\n").as_bytes(), 24, 3).expect("append");
        }

        let total = (0..3)
            .map(|index| {
                if index == 0 {
                    path.clone()
                } else {
                    rotated_path(&path, index)
                }
            })
            .filter_map(|candidate| fs::metadata(candidate).ok())
            .map(|metadata| metadata.len())
            .sum::<u64>();
        assert!(total <= 72);
        assert!(read_recent(&path, 256, 3).contains("line-7"));

        append(&path, b"after-restart\n", 24, 3).expect("append after restart");
        assert!(read_recent(&path, 256, 3).contains("after-restart"));
        let _ = fs::remove_dir_all(root);
    }
}
