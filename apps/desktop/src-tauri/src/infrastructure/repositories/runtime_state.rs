use rusqlite::{params, Connection, OptionalExtension};

use crate::local_db::{LocalResult, ManagedRuntimeState};

pub fn load_runtime_state(
    conn: &Connection,
    platform_id: &str,
    runtime_version: &str,
    python_version: &str,
) -> LocalResult<ManagedRuntimeState> {
    let loaded = conn
        .query_row(
            "SELECT platform_id, runtime_version, python_version, status,
                    python_executable_path, models_root, install_root, last_error,
                    installed_at, updated_at, last_log_path
             FROM runtime_state
             WHERE platform_id = ?1",
            params![platform_id],
            |row| {
                Ok(ManagedRuntimeState {
                    platform_id: row.get(0)?,
                    runtime_version: row.get(1)?,
                    python_version: row.get(2)?,
                    status: row.get(3)?,
                    python_executable_path: row.get(4)?,
                    models_root: row.get(5)?,
                    install_root: row.get(6)?,
                    last_error: row.get(7)?,
                    installed_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    last_log_path: row.get(10)?,
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())?;

    Ok(loaded.unwrap_or_else(|| {
        ManagedRuntimeState::missing(platform_id, runtime_version, python_version)
    }))
}

pub fn save_runtime_state(conn: &Connection, state: &ManagedRuntimeState) -> LocalResult<()> {
    conn.execute(
        "INSERT INTO runtime_state (
            platform_id, runtime_version, python_version, status, python_executable_path,
            models_root, install_root, last_error, installed_at, updated_at, last_log_path
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(platform_id) DO UPDATE SET
            runtime_version = excluded.runtime_version,
            python_version = excluded.python_version,
            status = excluded.status,
            python_executable_path = excluded.python_executable_path,
            models_root = excluded.models_root,
            install_root = excluded.install_root,
            last_error = excluded.last_error,
            installed_at = excluded.installed_at,
            updated_at = excluded.updated_at,
            last_log_path = excluded.last_log_path",
        params![
            state.platform_id,
            state.runtime_version,
            state.python_version,
            state.status,
            state.python_executable_path,
            state.models_root,
            state.install_root,
            state.last_error,
            state.installed_at,
            state.updated_at,
            state.last_log_path
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}
