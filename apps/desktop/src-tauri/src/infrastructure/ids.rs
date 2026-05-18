use crate::infrastructure::time::unix_timestamp_millis;

pub fn timestamped_id(prefix: &str) -> String {
    format!("{prefix}-{}", unix_timestamp_millis())
}

pub fn timestamped_indexed_id(prefix: &str, index: usize) -> String {
    format!("{}-{}-{index}", prefix, unix_timestamp_millis())
}
