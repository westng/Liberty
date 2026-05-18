use rusqlite::{params, Transaction};

use crate::{infrastructure::time::unix_timestamp_millis_string, local_db::LocalResult};

pub fn append_job_event_tx(
    tx: &Transaction<'_>,
    job_id: &str,
    event_type: &str,
    message: &str,
    metadata_json: Option<&str>,
) -> LocalResult<()> {
    let now = unix_timestamp_millis_string();
    let id = format!("job-event-{job_id}-{now}-{event_type}");
    tx.execute(
        "INSERT INTO job_events (id, job_id, event_type, message, metadata_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, job_id, event_type, message, metadata_json, now],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}
