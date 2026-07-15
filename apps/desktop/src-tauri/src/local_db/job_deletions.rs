use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use super::{JobDeletionOperation, JobDeletionPhase, LocalResult};

struct RawJobDeletionOperation {
    operation_id: String,
    job_id: String,
    trash_name: String,
    phase: String,
    runner_pid: Option<i64>,
    runner_process_identity: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
    last_error: Option<String>,
}

pub(super) fn prepare_tx(
    tx: &Transaction<'_>,
    operation_id: &str,
    job_id: &str,
    trash_name: &str,
    now_ms: u64,
) -> LocalResult<JobDeletionOperation> {
    let now_ms = sql_integer(now_ms, "删除操作时间")?;
    tx.execute(
        "INSERT INTO job_deletion_ops (
           operation_id, job_id, trash_name, phase, runner_pid,
           runner_process_identity, created_at_ms, updated_at_ms
         )
         SELECT ?1, jobs.id, ?3, 'prepared', job_runs.pid,
                job_runs.process_identity, ?4, ?4
         FROM jobs
         LEFT JOIN job_runs ON job_runs.job_id = jobs.id
         WHERE jobs.id = ?2
         ON CONFLICT(job_id) DO NOTHING",
        params![operation_id, job_id, trash_name, now_ms],
    )
    .map_err(|err| err.to_string())?;

    find_for_job(tx, job_id)?.ok_or_else(|| "没有找到这个任务。".to_string())
}

pub(super) fn list(conn: &Connection) -> LocalResult<Vec<JobDeletionOperation>> {
    let mut statement = conn
        .prepare(
            "SELECT operation_id, job_id, trash_name, phase, runner_pid,
                    runner_process_identity, created_at_ms, updated_at_ms, last_error
             FROM job_deletion_ops
             ORDER BY created_at_ms, operation_id",
        )
        .map_err(|err| err.to_string())?;
    let rows = statement
        .query_map([], read_raw_operation)
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    rows.into_iter().map(convert_operation).collect()
}

pub(super) fn find_for_job(
    conn: &Connection,
    job_id: &str,
) -> LocalResult<Option<JobDeletionOperation>> {
    let raw = conn
        .query_row(
            "SELECT operation_id, job_id, trash_name, phase, runner_pid,
                    runner_process_identity, created_at_ms, updated_at_ms, last_error
             FROM job_deletion_ops WHERE job_id = ?1",
            params![job_id],
            read_raw_operation,
        )
        .optional()
        .map_err(|err| err.to_string())?;
    raw.map(convert_operation).transpose()
}

pub(super) fn require(
    conn: &Connection,
    operation_id: &str,
    job_id: &str,
) -> LocalResult<JobDeletionOperation> {
    let operation =
        find_for_job(conn, job_id)?.ok_or_else(|| "删除操作日志不存在。".to_string())?;
    if operation.operation_id != operation_id {
        return Err("删除操作日志与任务不匹配。".into());
    }
    Ok(operation)
}

pub(super) fn set_phase(
    conn: &Connection,
    operation_id: &str,
    phase: JobDeletionPhase,
    now_ms: u64,
) -> LocalResult<()> {
    let updated = conn
        .execute(
            "UPDATE job_deletion_ops
             SET phase = ?2, updated_at_ms = ?3, last_error = NULL
             WHERE operation_id = ?1",
            params![
                operation_id,
                phase.as_str(),
                sql_integer(now_ms, "删除操作更新时间")?
            ],
        )
        .map_err(|err| err.to_string())?;
    if updated == 1 {
        Ok(())
    } else {
        Err("删除操作日志不存在。".into())
    }
}

pub(super) fn record_error(
    conn: &Connection,
    operation_id: &str,
    error: &str,
    now_ms: u64,
) -> LocalResult<()> {
    conn.execute(
        "UPDATE job_deletion_ops
         SET last_error = ?2, updated_at_ms = ?3
         WHERE operation_id = ?1",
        params![
            operation_id,
            error,
            sql_integer(now_ms, "删除操作更新时间")?
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub(super) fn finish(conn: &Connection, operation_id: &str) -> LocalResult<()> {
    conn.execute(
        "DELETE FROM job_deletion_ops WHERE operation_id = ?1",
        params![operation_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn read_raw_operation(row: &Row<'_>) -> rusqlite::Result<RawJobDeletionOperation> {
    Ok(RawJobDeletionOperation {
        operation_id: row.get(0)?,
        job_id: row.get(1)?,
        trash_name: row.get(2)?,
        phase: row.get(3)?,
        runner_pid: row.get(4)?,
        runner_process_identity: row.get(5)?,
        created_at_ms: row.get(6)?,
        updated_at_ms: row.get(7)?,
        last_error: row.get(8)?,
    })
}

fn convert_operation(raw: RawJobDeletionOperation) -> LocalResult<JobDeletionOperation> {
    Ok(JobDeletionOperation {
        operation_id: raw.operation_id,
        job_id: raw.job_id,
        trash_name: raw.trash_name,
        phase: JobDeletionPhase::from_str(&raw.phase)?,
        runner_pid: raw
            .runner_pid
            .map(|pid| u32::try_from(pid).map_err(|_| "删除操作 runner PID 无效。".to_string()))
            .transpose()?,
        runner_process_identity: raw.runner_process_identity,
        created_at_ms: u64::try_from(raw.created_at_ms)
            .map_err(|_| "删除操作创建时间无效。".to_string())?,
        updated_at_ms: u64::try_from(raw.updated_at_ms)
            .map_err(|_| "删除操作更新时间无效。".to_string())?,
        last_error: raw.last_error,
    })
}

fn sql_integer(value: u64, label: &str) -> LocalResult<i64> {
    i64::try_from(value).map_err(|_| format!("{label}超出 SQLite INTEGER 范围。"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_log_is_idempotent_and_keeps_runner_identity_after_job_delete() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE jobs (id TEXT PRIMARY KEY);
             CREATE TABLE job_runs (
               job_id TEXT PRIMARY KEY,
               pid INTEGER,
               process_identity TEXT
             );
             CREATE TABLE job_deletion_ops (
               operation_id TEXT PRIMARY KEY,
               job_id TEXT NOT NULL UNIQUE,
               trash_name TEXT NOT NULL UNIQUE,
               phase TEXT NOT NULL,
               runner_pid INTEGER,
               runner_process_identity TEXT,
               created_at_ms INTEGER NOT NULL,
               updated_at_ms INTEGER NOT NULL,
               last_error TEXT
             );
             INSERT INTO jobs(id) VALUES('job-1700000000000-1');
             INSERT INTO job_runs(job_id, pid, process_identity)
               VALUES('job-1700000000000-1', 42, 'identity-42');",
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        let first = prepare_tx(
            &tx,
            "delete-1",
            "job-1700000000000-1",
            "job-1700000000000-1-delete-1",
            10,
        )
        .unwrap();
        let repeated = prepare_tx(
            &tx,
            "delete-2",
            "job-1700000000000-1",
            "job-1700000000000-1-delete-2",
            20,
        )
        .unwrap();
        assert_eq!(first, repeated);
        assert_eq!(first.runner_pid, Some(42));
        assert_eq!(
            first.runner_process_identity.as_deref(),
            Some("identity-42")
        );
        tx.commit().unwrap();

        conn.execute("DELETE FROM jobs", []).unwrap();
        set_phase(&conn, "delete-1", JobDeletionPhase::DatabaseDeleted, 30).unwrap();
        let persisted = list(&conn).unwrap();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].phase, JobDeletionPhase::DatabaseDeleted);

        finish(&conn, "delete-1").unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}
