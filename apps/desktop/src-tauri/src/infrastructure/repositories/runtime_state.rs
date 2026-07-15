use rusqlite::{params, Connection, OptionalExtension};

use crate::local_db::{
    LocalResult, ManagedRuntimeState, RuntimeArtifactState, RuntimeComponentState,
    RuntimeOperationState,
};

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
                    ffmpeg_path: None,
                    last_error: row.get(7)?,
                    installed_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    last_log_path: row.get(10)?,
                    python: RuntimeComponentState::unavailable("python", Some("managed")),
                    ffmpeg: RuntimeComponentState::unavailable("ffmpeg", Some("managed")),
                    models: RuntimeComponentState::unavailable("model", None),
                    shell_ready: false,
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())?;

    Ok(loaded.unwrap_or_else(|| {
        ManagedRuntimeState::missing(platform_id, runtime_version, python_version)
    }))
}

pub fn load_runtime_component_state(
    conn: &Connection,
    platform_id: &str,
    component: &str,
    source: &str,
) -> LocalResult<RuntimeComponentState> {
    let loaded = conn
        .query_row(
            "SELECT availability, active_generation_id, artifact_version, resolved_path,
                    operation_kind, operation_generation, phase, progress, last_error, updated_at
             FROM runtime_component_state
             WHERE platform_id = ?1 AND component = ?2 AND source = ?3",
            params![platform_id, component, source],
            |row| {
                let generation_id = row.get::<_, Option<String>>(1)?;
                let artifact_version = row.get::<_, Option<String>>(2)?;
                let resolved_path = row.get::<_, Option<String>>(3)?;
                let active_artifact = match (generation_id, artifact_version, resolved_path) {
                    (Some(generation_id), Some(artifact_version), Some(resolved_path)) => {
                        Some(RuntimeArtifactState {
                            generation_id,
                            artifact_version,
                            resolved_path,
                        })
                    }
                    _ => None,
                };

                Ok(RuntimeComponentState {
                    component: component.to_string(),
                    source: (component != "model").then(|| source.to_string()),
                    availability: row.get(0)?,
                    active_artifact,
                    operation: RuntimeOperationState {
                        kind: row.get(4)?,
                        generation: row.get::<_, i64>(5)?.max(0) as u64,
                        phase: row.get(6)?,
                        progress: row
                            .get::<_, Option<i64>>(7)?
                            .map(|value| value.max(0) as u32),
                        last_error: row.get(8)?,
                    },
                    updated_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())?;

    Ok(loaded.unwrap_or_else(|| {
        RuntimeComponentState::unavailable(
            component,
            if component == "model" {
                None
            } else {
                Some(source)
            },
        )
    }))
}

pub fn save_runtime_component_state(
    conn: &Connection,
    platform_id: &str,
    state: &RuntimeComponentState,
) -> LocalResult<()> {
    let source = state.source.as_deref().unwrap_or("managed");
    let active_generation_id = state
        .active_artifact
        .as_ref()
        .map(|artifact| artifact.generation_id.as_str());
    let artifact_version = state
        .active_artifact
        .as_ref()
        .map(|artifact| artifact.artifact_version.as_str());
    let resolved_path = state
        .active_artifact
        .as_ref()
        .map(|artifact| artifact.resolved_path.as_str());

    conn.execute(
        "INSERT INTO runtime_component_state (
            platform_id, component, source, availability, active_generation_id,
            artifact_version, resolved_path, operation_kind, operation_generation,
            phase, progress, last_error, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(platform_id, component, source) DO UPDATE SET
            availability = excluded.availability,
            active_generation_id = excluded.active_generation_id,
            artifact_version = excluded.artifact_version,
            resolved_path = excluded.resolved_path,
            operation_kind = excluded.operation_kind,
            operation_generation = excluded.operation_generation,
            phase = excluded.phase,
            progress = excluded.progress,
            last_error = excluded.last_error,
            updated_at = excluded.updated_at",
        params![
            platform_id,
            state.component,
            source,
            state.availability,
            active_generation_id,
            artifact_version,
            resolved_path,
            state.operation.kind,
            i64::try_from(state.operation.generation).unwrap_or(i64::MAX),
            state.operation.phase,
            state.operation.progress.map(i64::from),
            state.operation.last_error,
            state.updated_at,
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{load_runtime_component_state, save_runtime_component_state};
    use crate::local_db::{RuntimeArtifactState, RuntimeComponentState, RuntimeOperationState};
    use rusqlite::Connection;

    #[test]
    fn component_states_round_trip_without_cross_component_overwrite() {
        let conn = Connection::open_in_memory().expect("database");
        conn.execute_batch(
            "CREATE TABLE runtime_component_state (
                platform_id TEXT NOT NULL,
                component TEXT NOT NULL,
                source TEXT NOT NULL,
                availability TEXT NOT NULL,
                active_generation_id TEXT,
                artifact_version TEXT,
                resolved_path TEXT,
                operation_kind TEXT NOT NULL,
                operation_generation INTEGER NOT NULL,
                phase TEXT NOT NULL,
                progress INTEGER,
                last_error TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(platform_id, component, source)
            );",
        )
        .expect("schema");

        let python = RuntimeComponentState {
            component: "python".into(),
            source: Some("system".into()),
            availability: "ready".into(),
            active_artifact: Some(RuntimeArtifactState {
                generation_id: "system-4".into(),
                artifact_version: "system".into(),
                resolved_path: "/usr/local/bin/python3".into(),
            }),
            operation: RuntimeOperationState {
                kind: "idle".into(),
                generation: 4,
                phase: "ready".into(),
                progress: Some(100),
                last_error: None,
            },
            updated_at: "4".into(),
        };
        let models = RuntimeComponentState {
            component: "model".into(),
            source: None,
            availability: "unavailable".into(),
            active_artifact: None,
            operation: RuntimeOperationState {
                kind: "waiting_for_python".into(),
                generation: 9,
                phase: "waiting_for_python".into(),
                progress: None,
                last_error: None,
            },
            updated_at: "9".into(),
        };

        save_runtime_component_state(&conn, "darwin-aarch64", &python).expect("save python");
        save_runtime_component_state(&conn, "darwin-aarch64", &models).expect("save models");

        let loaded_python =
            load_runtime_component_state(&conn, "darwin-aarch64", "python", "system")
                .expect("load python");
        let loaded_models =
            load_runtime_component_state(&conn, "darwin-aarch64", "model", "managed")
                .expect("load models");
        assert_eq!(loaded_python.availability, "ready");
        assert_eq!(loaded_python.operation.generation, 4);
        assert_eq!(
            loaded_python
                .active_artifact
                .expect("active python")
                .resolved_path,
            "/usr/local/bin/python3"
        );
        assert_eq!(loaded_models.operation.kind, "waiting_for_python");
        assert_eq!(loaded_models.operation.generation, 9);
        assert!(loaded_models.source.is_none());
    }
}
