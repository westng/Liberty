use std::{
    fs::{self, OpenOptions},
    io::Write,
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

pub fn read_trimmed(log_dir: &Path) -> String {
    fs::read_to_string(log_dir.join("process.log"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub fn summarize_tail(log_dir: &Path, max_lines: usize) -> Option<String> {
    let log = fs::read_to_string(log_dir.join("process.log")).ok()?;
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
