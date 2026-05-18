use std::time::{SystemTime, UNIX_EPOCH};

pub fn unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub fn unix_timestamp_millis_string() -> String {
    unix_timestamp_millis().to_string()
}
