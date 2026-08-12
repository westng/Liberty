use std::path::Path;

use crate::{
    infrastructure::observability::rotating_log::{
        self, DEFAULT_FILE_COUNT, DEFAULT_FILE_LIMIT_BYTES,
    },
    local_db::LocalResult,
};

pub fn append_bytes(log_dir: &Path, bytes: &[u8]) -> LocalResult<()> {
    if bytes.is_empty() {
        return Ok(());
    }

    let log_path = log_dir.join("process.log");
    rotating_log::append(
        &log_path,
        bytes,
        DEFAULT_FILE_LIMIT_BYTES,
        DEFAULT_FILE_COUNT,
    )
    .map_err(|err| err.to_string())
}

pub fn append_line(log_dir: &Path, line: &str) -> LocalResult<()> {
    append_bytes(log_dir, format!("{line}\n").as_bytes())
}

pub fn reset(log_dir: &Path) -> LocalResult<()> {
    rotating_log::reset(&log_dir.join("process.log"), DEFAULT_FILE_COUNT)
        .map_err(|err| err.to_string())
}

pub fn read_recent(log_dir: &Path, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }

    rotating_log::read_recent(&log_dir.join("process.log"), max_bytes, DEFAULT_FILE_COUNT)
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
    use std::fs;

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
