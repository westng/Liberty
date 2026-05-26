use crate::infrastructure::time::unix_timestamp_millis;
use std::sync::atomic::{AtomicU64, Ordering};

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn timestamped_id(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        unix_timestamp_millis(),
        ID_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn timestamped_indexed_id(prefix: &str, index: usize) -> String {
    format!(
        "{}-{}-{}-{index}",
        prefix,
        unix_timestamp_millis(),
        ID_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
