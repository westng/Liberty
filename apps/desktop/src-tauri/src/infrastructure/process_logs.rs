use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::local_db::LocalResult;

pub fn append_bytes(log_dir: &Path, bytes: &[u8]) -> LocalResult<()> {
    if bytes.is_empty() {
        return Ok(());
    }

    let log_path = log_dir.join("process.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|err| err.to_string())?;
    file.write_all(bytes).map_err(|err| err.to_string())
}

pub fn append_line(log_dir: &Path, line: &str) -> LocalResult<()> {
    append_bytes(log_dir, format!("{line}\n").as_bytes())
}

pub fn reset(log_dir: &Path) -> LocalResult<()> {
    fs::write(log_dir.join("process.log"), []).map_err(|err| err.to_string())
}

pub fn read_recent(log_dir: &Path, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }

    let Ok(mut file) = File::open(log_dir.join("process.log")) else {
        return String::new();
    };
    let Ok(file_size) = file.metadata().map(|metadata| metadata.len()) else {
        return String::new();
    };
    let read_size = file_size.min(max_bytes as u64);
    let start = file_size.saturating_sub(read_size);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }

    let mut bytes = vec![0; read_size as usize];
    if file.read_exact(&mut bytes).is_err() {
        return String::new();
    }
    let mut content = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0 {
        if let Some(first_newline) = content.find('\n') {
            content.drain(..=first_newline);
        }
    }
    content.trim().to_string()
}

pub fn summarize_tail(log_dir: &Path, max_lines: usize) -> Option<String> {
    let log = read_recent(log_dir, 256 * 1024);
    let lines: Vec<&str> = log.lines().filter(|line| !line.trim().is_empty()).collect();

    if lines.is_empty() {
        return None;
    }

    let tail = lines
        .iter()
        .rev()
        .take(max_lines)
        .copied()
        .collect::<Vec<_>>();
    Some(tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_complete_lines_from_bounded_tail() {
        let root = std::env::temp_dir().join(format!(
            "liberty-process-logs-{}-{}",
            std::process::id(),
            crate::infrastructure::time::unix_timestamp_millis()
        ));
        fs::create_dir_all(&root).expect("log directory");
        fs::write(
            root.join("process.log"),
            "first line\nsecond line\nthird line\n",
        )
        .expect("log fixture");

        assert_eq!(read_recent(&root, 24), "second line\nthird line");
        assert_eq!(summarize_tail(&root, 1).as_deref(), Some("third line"));

        let _ = fs::remove_dir_all(root);
    }
}
