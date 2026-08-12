use crate::{
    application::complete_asr_job::{AsrJobCompletion, AsrJobCompletionPort},
    infrastructure::{
        ids, migrations,
        repositories::{
            ai_models, ai_summary_runs, ai_templates, farm, job_events, members, pet,
            pet_blind_box, pet_check_in, pet_redeem_key, pet_store, runtime_state, settings,
            work_game,
        },
    },
};
use rusqlite::{params, Connection, Transaction, TransactionBehavior};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tauri::{AppHandle, Manager};

pub use crate::domain::settings::{AppSettings, AppSettingsSnapshot};
pub use crate::domain::transcript::TranscriptSegment;

pub type LocalResult<T> = Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalJobRunRecord {
    pub job_id: String,
    pub attempt_id: u64,
    pub lease_token: u64,
    pub status: String,
    pub pid: Option<u32>,
    pub process_identity: Option<String>,
    pub heartbeat_at_ms: Option<u64>,
    pub started_at_ms: Option<u64>,
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalJobRunLease {
    pub attempt_id: u64,
    pub lease_token: u64,
    pub output_dir: String,
}

const MAX_DAILY_INTERACTION_PER_SOURCE: i64 = 20;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

static INITIALIZED_DATABASES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

pub use model::*;

mod job_deletion_model;
mod job_deletions;
mod jobs;
mod legacy;
mod model;
mod pet_growth;
pub(crate) mod pet_leveling;
mod progress;
pub(crate) mod schema;

pub use job_deletion_model::{JobDeletionOperation, JobDeletionPhase};

pub fn init_database(app: &AppHandle) -> LocalResult<()> {
    let path = database_path(app)?;
    let mut initialized = INITIALIZED_DATABASES
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .map_err(|err| format!("本地数据库初始化锁异常: {err}"))?;
    if initialized.contains(&path) {
        return Ok(());
    }

    let mut conn = open_connection_at_path(&path)?;
    schema::apply_schema(&conn)?;
    migrations::ensure_schema_version(&conn)?;
    schema::seed_builtin_templates(&conn)?;
    legacy::import_legacy_jobs(app, &mut conn)?;
    initialized.insert(path);
    Ok(())
}

pub fn open_connection(app: &AppHandle) -> LocalResult<Connection> {
    let path = database_path(app)?;
    open_connection_at_path(&path)
}

fn open_connection_at_path(path: &Path) -> LocalResult<Connection> {
    let conn = Connection::open(path)
        .map_err(|err| format!("无法打开本地数据库 {}: {err}", path.display()))?;
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)
        .map_err(|err| format!("无法设置本地数据库等待超时 {}: {err}", path.display()))?;
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA synchronous = NORMAL;
        ",
    )
    .map_err(|err| format!("无法初始化本地数据库连接 {}: {err}", path.display()))?;
    Ok(conn)
}

pub fn database_path(app: &AppHandle) -> LocalResult<PathBuf> {
    let data_root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| format!("无法定位本地数据目录: {err}"))?;
    fs::create_dir_all(&data_root)
        .map_err(|err| format!("无法创建本地数据目录 {}: {err}", data_root.display()))?;
    Ok(data_root.join("liberty.sqlite3"))
}

pub fn jobs_root(app: &AppHandle) -> LocalResult<PathBuf> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| err.to_string())?
        .join("jobs");
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    Ok(root)
}

pub fn list_jobs(app: &AppHandle) -> LocalResult<Vec<MeetingJob>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id FROM jobs
             WHERE NOT EXISTS (
               SELECT 1 FROM job_deletion_ops WHERE job_deletion_ops.job_id = jobs.id
             )
             ORDER BY datetime(created_at) DESC, created_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;

    ids.into_iter()
        .map(|id| jobs::load_job_summary(app, &conn, &id))
        .collect::<LocalResult<Vec<_>>>()
}

pub fn get_job(app: &AppHandle, job_id: &str) -> LocalResult<MeetingJob> {
    init_database(app)?;
    let conn = open_connection(app)?;
    jobs::load_job(app, &conn, job_id)
}

pub fn get_settings(app: &AppHandle) -> LocalResult<AppSettings> {
    init_database(app)?;
    let conn = open_connection(app)?;
    settings::load_settings(&conn)
}

pub fn get_settings_snapshot(app: &AppHandle) -> LocalResult<AppSettingsSnapshot> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let (settings, settings_revision) = settings::load_settings_with_revision(&conn)?;
    Ok(AppSettingsSnapshot {
        settings,
        settings_revision: Some(settings_revision),
    })
}

pub fn get_ui_preferences(app: &AppHandle) -> LocalResult<UiPreferences> {
    init_database(app)?;
    let conn = open_connection(app)?;
    settings::load_ui_preferences(&conn)
}

pub fn set_runtime_component_source(
    app: &AppHandle,
    component: &str,
    source: &str,
) -> LocalResult<i64> {
    init_database(app)?;
    let conn = open_connection(app)?;
    settings::set_runtime_component_source(&conn, component, source)
}

pub fn publish_detected_runtime_path(
    app: &AppHandle,
    platform_id: &str,
    state: &RuntimeComponentState,
    path: &str,
    expected_source: &str,
    expected_generation: u64,
) -> LocalResult<Option<i64>> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let revision = settings::publish_detected_runtime_path(
        &tx,
        platform_id,
        &state.component,
        path,
        expected_source,
        expected_generation,
    )?;
    if revision.is_none() {
        return Ok(None);
    }
    runtime_state::save_runtime_component_state(&tx, platform_id, state)?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(revision)
}

pub fn save_settings_snapshot(
    app: &AppHandle,
    snapshot: &AppSettingsSnapshot,
) -> LocalResult<AppSettingsSnapshot> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let expected_revision = match snapshot.settings_revision {
        Some(revision) => revision,
        None => settings::load_settings_with_revision(&conn)?.1,
    };
    settings::save_settings_if_revision(&conn, &snapshot.settings, expected_revision)?;
    let (settings, settings_revision) = settings::load_settings_with_revision(&conn)?;
    Ok(AppSettingsSnapshot {
        settings,
        settings_revision: Some(settings_revision),
    })
}

pub fn get_runtime_state(
    app: &AppHandle,
    platform_id: &str,
    runtime_version: &str,
    python_version: &str,
) -> LocalResult<ManagedRuntimeState> {
    init_database(app)?;
    let conn = open_connection(app)?;
    runtime_state::load_runtime_state(&conn, platform_id, runtime_version, python_version)
}

pub fn save_runtime_state(app: &AppHandle, state: &ManagedRuntimeState) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    runtime_state::save_runtime_state(&conn, state)
}

pub fn get_runtime_component_state(
    app: &AppHandle,
    platform_id: &str,
    component: &str,
    source: &str,
) -> LocalResult<RuntimeComponentState> {
    init_database(app)?;
    let conn = open_connection(app)?;
    runtime_state::load_runtime_component_state(&conn, platform_id, component, source)
}

pub fn save_runtime_component_state(
    app: &AppHandle,
    platform_id: &str,
    state: &RuntimeComponentState,
) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    runtime_state::save_runtime_component_state(&conn, platform_id, state)
}

pub fn get_work_market_state(app: &AppHandle) -> LocalResult<WorkMarketState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let state = farm::work_market_state_tx(&tx, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(state)
}

pub fn get_farm_state(app: &AppHandle) -> LocalResult<FarmState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let state = farm::state_tx(&tx, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(state)
}

pub fn plant_farm_crop(app: &AppHandle, plot_id: &str, crop_key: &str) -> LocalResult<FarmState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let state = farm::plant_crop_tx(&tx, plot_id, crop_key, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(state)
}

pub fn water_farm_plot(app: &AppHandle, plot_id: &str) -> LocalResult<FarmState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let state = farm::water_plot_tx(&tx, plot_id, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(state)
}

pub fn harvest_farm_plot(app: &AppHandle, plot_id: &str) -> LocalResult<FarmHarvestResult> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let (state, harvest) = farm::harvest_plot_tx(&tx, plot_id, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(FarmHarvestResult { state, harvest })
}

pub fn list_farm_harvest_ledger(
    app: &AppHandle,
    limit: usize,
) -> LocalResult<Vec<FarmHarvestLedgerEntry>> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    farm::state_tx(&tx, &now)?;
    let entries = farm::list_harvest_ledger_tx(&tx, limit)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(entries)
}

pub fn get_work_game_state(app: &AppHandle, game_key: &str) -> LocalResult<WorkGameState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let state = work_game::state_tx(&tx, game_key, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(state)
}

pub fn start_work_game_task(
    app: &AppHandle,
    game_key: &str,
    task_id: &str,
    job_key: &str,
) -> LocalResult<WorkGameState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let state = work_game::start_task_tx(&tx, game_key, task_id, job_key, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(state)
}

pub fn care_work_game_task(
    app: &AppHandle,
    game_key: &str,
    task_id: &str,
) -> LocalResult<WorkGameState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let state = work_game::care_task_tx(&tx, game_key, task_id, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(state)
}

pub fn claim_work_game_task(
    app: &AppHandle,
    game_key: &str,
    task_id: &str,
) -> LocalResult<WorkGameClaimResult> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let result = work_game::claim_task_tx(&tx, game_key, task_id, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(result)
}

pub fn save_job_snapshot(app: &AppHandle, job: &MeetingJob) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    jobs::save_job_snapshot_tx(&tx, job)?;
    if job.source == "local" {
        insert_queued_job_run_tx(&tx, &job.id)?;
    }
    job_events::append_job_event_tx(&tx, &job.id, "created", "任务已创建。", None)?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn list_recoverable_local_job_runs(app: &AppHandle) -> LocalResult<Vec<LocalJobRunRecord>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ensure_recoverable_job_run_rows(&conn)?;
    list_recoverable_local_job_runs_on_connection(&conn)
}

pub fn begin_local_job_run(
    app: &AppHandle,
    job_id: &str,
    started_at_ms: u64,
) -> LocalResult<LocalJobRunLease> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    begin_local_job_run_on_connection(&mut conn, job_id, started_at_ms)
}

pub fn requeue_recovered_local_job_run(
    app: &AppHandle,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
) -> LocalResult<bool> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    requeue_recovered_local_job_run_on_connection(&mut conn, job_id, attempt_id, lease_token)
}

pub fn fence_local_job_run(app: &AppHandle, job_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| err.to_string())?;
    tx.execute(
        "INSERT INTO job_runs (job_id, attempt_id, lease_token, status)
         VALUES (?1, 0, 1, 'fenced')
         ON CONFLICT(job_id) DO UPDATE SET
           lease_token = job_runs.lease_token + 1,
           status = 'fenced',
           pid = NULL,
           process_identity = NULL,
           heartbeat_at_ms = NULL,
           finished_at_ms = NULL,
           output_dir = NULL",
        params![job_id],
    )
    .map_err(|err| err.to_string())?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn attach_local_job_process(
    app: &AppHandle,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
    pid: u32,
    process_identity: &str,
    heartbeat_at_ms: u64,
) -> LocalResult<bool> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let updated = conn
        .execute(
            "UPDATE job_runs
             SET pid = ?4,
                 process_identity = ?5,
                 heartbeat_at_ms = ?6
             WHERE job_id = ?1
               AND attempt_id = ?2
               AND lease_token = ?3
               AND status = 'running'",
            params![
                job_id,
                as_sql_i64(attempt_id, "attempt_id")?,
                as_sql_i64(lease_token, "lease_token")?,
                i64::from(pid),
                process_identity,
                as_sql_i64(heartbeat_at_ms, "heartbeat_at_ms")?
            ],
        )
        .map_err(|err| err.to_string())?;
    Ok(updated == 1)
}

pub fn heartbeat_local_job_run(
    app: &AppHandle,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
    pid: Option<u32>,
    heartbeat_at_ms: u64,
) -> LocalResult<bool> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let updated = conn
        .execute(
            "UPDATE job_runs
             SET heartbeat_at_ms = ?5
             WHERE job_id = ?1
               AND attempt_id = ?2
               AND lease_token = ?3
               AND status = 'running'
               AND (?4 IS NULL OR pid = ?4)",
            params![
                job_id,
                as_sql_i64(attempt_id, "attempt_id")?,
                as_sql_i64(lease_token, "lease_token")?,
                pid.map(i64::from),
                as_sql_i64(heartbeat_at_ms, "heartbeat_at_ms")?
            ],
        )
        .map_err(|err| err.to_string())?;
    Ok(updated == 1)
}

pub fn is_current_local_job_run(
    app: &AppHandle,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
) -> LocalResult<bool> {
    init_database(app)?;
    let conn = open_connection(app)?;
    is_current_local_job_run_on_connection(&conn, job_id, attempt_id, lease_token)
}

pub fn update_local_job_process_log(
    app: &AppHandle,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
    process_log: &str,
) -> LocalResult<bool> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let updated = conn
        .execute(
            "UPDATE jobs
             SET process_log = ?4
             WHERE id = ?1
               AND EXISTS (
                 SELECT 1 FROM job_runs
                 WHERE job_id = ?1
                   AND attempt_id = ?2
                   AND lease_token = ?3
                   AND status = 'running'
               )",
            params![
                job_id,
                as_sql_i64(attempt_id, "attempt_id")?,
                as_sql_i64(lease_token, "lease_token")?,
                process_log
            ],
        )
        .map_err(|err| err.to_string())?;
    Ok(updated == 1)
}

pub struct SqliteAsrJobCompletion<'app> {
    app: &'app AppHandle,
}

impl<'app> SqliteAsrJobCompletion<'app> {
    pub fn new(app: &'app AppHandle) -> Self {
        Self { app }
    }
}

impl AsrJobCompletionPort for SqliteAsrJobCompletion<'_> {
    fn complete(&self, completion: &AsrJobCompletion) -> LocalResult<bool> {
        init_database(self.app)?;
        let mut conn = open_connection(self.app)?;
        complete_local_job_run_on_connection(&mut conn, completion)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn fail_local_job_run(
    app: &AppHandle,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
    processing_finished_at_ms: u64,
    processing_duration_seconds: Option<u32>,
    failure_reason: &str,
    process_log: &str,
) -> LocalResult<bool> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    fail_local_job_run_on_connection(
        &mut conn,
        job_id,
        attempt_id,
        lease_token,
        processing_finished_at_ms,
        processing_duration_seconds,
        failure_reason,
        process_log,
    )
}

pub fn local_job_run_has_status(
    app: &AppHandle,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
    status: &str,
) -> LocalResult<bool> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let exists = conn
        .query_row(
            "SELECT EXISTS (
               SELECT 1 FROM job_runs
               WHERE job_id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = ?4
             )",
            params![
                job_id,
                as_sql_i64(attempt_id, "attempt_id")?,
                as_sql_i64(lease_token, "lease_token")?,
                status
            ],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|err| err.to_string())?;
    Ok(exists)
}

fn insert_queued_job_run_tx(tx: &Transaction<'_>, job_id: &str) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO job_runs (job_id, attempt_id, lease_token, status)
         VALUES (?1, 0, 0, 'queued')
         ON CONFLICT(job_id) DO UPDATE SET
           status = 'queued',
           pid = NULL,
           process_identity = NULL,
           heartbeat_at_ms = NULL,
           started_at_ms = NULL,
           finished_at_ms = NULL,
           output_dir = NULL,
           last_error = NULL",
        params![job_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn ensure_recoverable_job_run_rows(conn: &Connection) -> LocalResult<()> {
    conn.execute(
        "INSERT INTO job_runs (
           job_id, attempt_id, lease_token, status, started_at_ms, output_dir
         )
         SELECT id,
                CASE WHEN overall_status = 'queued' THEN 0 ELSE 1 END,
                CASE WHEN overall_status = 'queued' THEN 0 ELSE 1 END,
                CASE WHEN overall_status = 'queued' THEN 'queued' ELSE 'running' END,
                processing_started_at_ms,
                CASE
                  WHEN overall_status = 'queued' THEN NULL
                  ELSE 'attempts/attempt-1-1'
                END
         FROM jobs
         WHERE overall_status IN ('queued', 'transcribing', 'speaker_processing')
           AND NOT EXISTS (
             SELECT 1 FROM job_deletion_ops WHERE job_deletion_ops.job_id = jobs.id
           )
           AND NOT EXISTS (SELECT 1 FROM job_runs WHERE job_runs.job_id = jobs.id)",
        [],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn list_recoverable_local_job_runs_on_connection(
    conn: &Connection,
) -> LocalResult<Vec<LocalJobRunRecord>> {
    let mut statement = conn
        .prepare(
            "SELECT job_id, attempt_id, lease_token, status, pid, process_identity,
                    heartbeat_at_ms, started_at_ms, output_dir
             FROM job_runs
             WHERE status IN ('queued', 'running')
               AND NOT EXISTS (
                 SELECT 1 FROM job_deletion_ops
                 WHERE job_deletion_ops.job_id = job_runs.job_id
               )
             ORDER BY COALESCE(started_at_ms, 0), job_id",
        )
        .map_err(|err| err.to_string())?;
    let runs = statement
        .query_map([], |row| {
            Ok(LocalJobRunRecord {
                job_id: row.get(0)?,
                attempt_id: row.get::<_, i64>(1)? as u64,
                lease_token: row.get::<_, i64>(2)? as u64,
                status: row.get(3)?,
                pid: row.get::<_, Option<i64>>(4)?.map(|value| value as u32),
                process_identity: row.get(5)?,
                heartbeat_at_ms: row.get::<_, Option<i64>>(6)?.map(|value| value as u64),
                started_at_ms: row.get::<_, Option<i64>>(7)?.map(|value| value as u64),
                output_dir: row.get(8)?,
            })
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(runs)
}

fn begin_local_job_run_on_connection(
    conn: &mut Connection,
    job_id: &str,
    started_at_ms: u64,
) -> LocalResult<LocalJobRunLease> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| err.to_string())?;
    let started_at = as_sql_i64(started_at_ms, "started_at_ms")?;
    let (attempt_id, lease_token) = tx
        .query_row(
            "INSERT INTO job_runs (
               job_id, attempt_id, lease_token, status, started_at_ms, heartbeat_at_ms,
               pid, process_identity, finished_at_ms, output_dir, last_error
             ) VALUES (?1, 1, 1, 'running', ?2, ?2, NULL, NULL, NULL, NULL, NULL)
             ON CONFLICT(job_id) DO UPDATE SET
               attempt_id = job_runs.attempt_id + 1,
               lease_token = job_runs.lease_token + 1,
               status = 'running',
               started_at_ms = excluded.started_at_ms,
               heartbeat_at_ms = excluded.heartbeat_at_ms,
               pid = NULL,
               process_identity = NULL,
               finished_at_ms = NULL,
               output_dir = NULL,
               last_error = NULL
             RETURNING attempt_id, lease_token",
            params![job_id, started_at],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|err| err.to_string())?;
    let output_dir = format!("attempts/attempt-{attempt_id}-{lease_token}");
    tx.execute(
        "UPDATE job_runs SET output_dir = ?2 WHERE job_id = ?1",
        params![job_id, output_dir],
    )
    .map_err(|err| err.to_string())?;
    let updated = tx
        .execute(
            "UPDATE jobs
             SET asr_status = 'transcribing',
                 summary_status = 'idle',
                 overall_status = 'transcribing',
                 diarization_status = CASE
                   WHEN enable_speaker = 1 THEN 'processing'
                   ELSE 'disabled'
                 END,
                 warnings_json = '[]',
                 failure_reason = NULL,
                 processing_started_at_ms = ?2,
                 processing_finished_at_ms = NULL,
                 processing_duration_seconds = NULL
             WHERE id = ?1",
            params![job_id, started_at],
        )
        .map_err(|err| err.to_string())?;
    if updated != 1 {
        return Err("没有找到这个任务。".into());
    }
    job_events::append_job_event_tx(&tx, job_id, "started", "任务开始处理。", None)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(LocalJobRunLease {
        attempt_id: attempt_id as u64,
        lease_token: lease_token as u64,
        output_dir,
    })
}

fn requeue_recovered_local_job_run_on_connection(
    conn: &mut Connection,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
) -> LocalResult<bool> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| err.to_string())?;
    let updated = tx
        .execute(
            "UPDATE job_runs
             SET lease_token = lease_token + 1,
                 status = 'queued',
                 pid = NULL,
                 process_identity = NULL,
                 heartbeat_at_ms = NULL,
                 started_at_ms = NULL,
                 finished_at_ms = NULL,
                 output_dir = NULL,
                 last_error = NULL
             WHERE job_id = ?1 AND attempt_id = ?2 AND lease_token = ?3",
            params![
                job_id,
                as_sql_i64(attempt_id, "attempt_id")?,
                as_sql_i64(lease_token, "lease_token")?
            ],
        )
        .map_err(|err| err.to_string())?;
    if updated == 1 {
        tx.execute(
            "UPDATE jobs
             SET asr_status = 'queued', summary_status = 'idle', overall_status = 'queued',
                 diarization_status = CASE
                   WHEN enable_speaker = 1 THEN 'pending'
                   ELSE 'disabled'
                 END,
                 warnings_json = '[]',
                 failure_reason = NULL, processing_started_at_ms = NULL,
                 processing_finished_at_ms = NULL, processing_duration_seconds = NULL
             WHERE id = ?1",
            params![job_id],
        )
        .map_err(|err| err.to_string())?;
    }
    tx.commit().map_err(|err| err.to_string())?;
    Ok(updated == 1)
}

fn is_current_local_job_run_on_connection(
    conn: &Connection,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
) -> LocalResult<bool> {
    conn.query_row(
        "SELECT EXISTS (
           SELECT 1 FROM job_runs
           WHERE job_id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'
         )",
        params![
            job_id,
            as_sql_i64(attempt_id, "attempt_id")?,
            as_sql_i64(lease_token, "lease_token")?
        ],
        |row| row.get::<_, bool>(0),
    )
    .map_err(|err| err.to_string())
}

fn complete_local_job_run_on_connection(
    conn: &mut Connection,
    completion: &AsrJobCompletion,
) -> LocalResult<bool> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| err.to_string())?;
    if !claim_current_run_tx(
        &tx,
        &completion.job_id,
        completion.attempt_id,
        completion.lease_token,
        "completed",
    )? {
        return Ok(false);
    }
    jobs::replace_segments_tx(
        &tx,
        &completion.job_id,
        "transcript",
        &completion.result.transcript_segments,
    )?;
    jobs::replace_segments_tx(
        &tx,
        &completion.job_id,
        "speaker",
        &completion.result.speaker_segments,
    )?;
    let warnings_json = serde_json::to_string(&completion.result.warnings)
        .map_err(|error| format!("无法序列化 Runner warnings: {error}"))?;
    tx.execute(
        "UPDATE jobs
         SET duration_minutes = ?2,
             processing_finished_at_ms = ?3,
             processing_duration_seconds = ?4,
             runner_protocol_version = ?5,
             asr_backend = ?6,
             diarization_status = ?7,
             warnings_json = ?8,
             summary_status = 'idle',
             asr_status = 'completed',
             overall_status = 'completed',
             failure_reason = NULL,
             process_log = ?9
         WHERE id = ?1",
        params![
            completion.job_id,
            i64::from(completion.duration_minutes),
            as_sql_i64(
                completion.processing_finished_at_ms,
                "processing_finished_at_ms"
            )?,
            completion.processing_duration_seconds.map(i64::from),
            i64::from(completion.result.protocol_version),
            completion.result.asr_backend.as_str(),
            completion.result.diarization_status.as_str(),
            warnings_json,
            completion.process_log
        ],
    )
    .map_err(|err| err.to_string())?;
    job_events::append_job_event_tx(&tx, &completion.job_id, "completed", "任务处理完成。", None)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn fail_local_job_run_on_connection(
    conn: &mut Connection,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
    processing_finished_at_ms: u64,
    processing_duration_seconds: Option<u32>,
    failure_reason: &str,
    process_log: &str,
) -> LocalResult<bool> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| err.to_string())?;
    if !claim_current_run_tx(&tx, job_id, attempt_id, lease_token, "failed")? {
        return Ok(false);
    }
    tx.execute(
        "UPDATE job_runs SET last_error = ?4 WHERE job_id = ?1 AND attempt_id = ?2 AND lease_token = ?3",
        params![
            job_id,
            as_sql_i64(attempt_id, "attempt_id")?,
            as_sql_i64(lease_token, "lease_token")?,
            failure_reason
        ],
    )
    .map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE jobs
         SET processing_finished_at_ms = ?2,
             processing_duration_seconds = ?3,
             asr_status = 'failed',
             summary_status = 'idle',
             overall_status = 'failed',
             diarization_status = CASE
               WHEN enable_speaker = 1 THEN 'failed'
               ELSE 'disabled'
             END,
             failure_reason = ?4,
             process_log = ?5
         WHERE id = ?1",
        params![
            job_id,
            as_sql_i64(processing_finished_at_ms, "processing_finished_at_ms")?,
            processing_duration_seconds.map(i64::from),
            failure_reason,
            process_log
        ],
    )
    .map_err(|err| err.to_string())?;
    job_events::append_job_event_tx(&tx, job_id, "failed", failure_reason, None)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(true)
}

fn claim_current_run_tx(
    tx: &Transaction<'_>,
    job_id: &str,
    attempt_id: u64,
    lease_token: u64,
    terminal_status: &str,
) -> LocalResult<bool> {
    let updated = tx
        .execute(
            "UPDATE job_runs
             SET status = ?4, finished_at_ms = ?5, pid = NULL, heartbeat_at_ms = NULL
             WHERE job_id = ?1 AND attempt_id = ?2 AND lease_token = ?3 AND status = 'running'",
            params![
                job_id,
                as_sql_i64(attempt_id, "attempt_id")?,
                as_sql_i64(lease_token, "lease_token")?,
                terminal_status,
                as_sql_i64(
                    crate::infrastructure::time::unix_timestamp_millis() as u64,
                    "finished_at_ms"
                )?
            ],
        )
        .map_err(|err| err.to_string())?;
    Ok(updated == 1)
}

fn as_sql_i64(value: u64, label: &str) -> LocalResult<i64> {
    i64::try_from(value).map_err(|_| format!("{label} 超出 SQLite INTEGER 范围。"))
}

pub fn rename_job_speaker(
    app: &AppHandle,
    job_id: &str,
    from_speaker: &str,
    to_speaker: &str,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let normalized_from = from_speaker.trim();
    let normalized_to = to_speaker.trim();

    if normalized_to.is_empty() {
        return Err("讲话人名称不能为空。".into());
    }

    if normalized_from.is_empty() {
        tx.execute(
            "UPDATE transcript_segments
             SET speaker = ?2
             WHERE job_id = ?1
               AND segment_type = 'speaker'
               AND (speaker IS NULL OR trim(speaker) = '')",
            params![job_id, normalized_to],
        )
        .map_err(|err| err.to_string())?;
    } else {
        tx.execute(
            "UPDATE transcript_segments
             SET speaker = ?3
             WHERE job_id = ?1
               AND segment_type = 'speaker'
               AND speaker = ?2",
            params![job_id, normalized_from, normalized_to],
        )
        .map_err(|err| err.to_string())?;
    }

    tx.commit().map_err(|err| err.to_string())
}

pub fn reset_job_for_retry(
    app: &AppHandle,
    job_id: &str,
    python_path: &str,
    runner_script_path: &str,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    reset_job_for_retry_on_connection(&mut conn, job_id, python_path, runner_script_path)
}

fn reset_job_for_retry_on_connection(
    conn: &mut Connection,
    job_id: &str,
    python_path: &str,
    runner_script_path: &str,
) -> LocalResult<()> {
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE jobs
         SET upload_status = ?2,
             asr_status = ?3,
             summary_status = ?4,
             overall_status = ?5,
             runner_protocol_version = NULL,
             asr_backend = 'unknown',
             diarization_status = CASE
               WHEN enable_speaker = 1 THEN 'pending'
               ELSE 'disabled'
             END,
             warnings_json = '[]',
             failure_reason = NULL,
             process_log = NULL,
             duration_minutes = 0,
             processing_started_at_ms = NULL,
             processing_finished_at_ms = NULL,
             processing_duration_seconds = NULL,
             python_path = ?6,
             runner_script_path = ?7
         WHERE id = ?1",
        params![
            job_id,
            "uploaded",
            "queued",
            "idle",
            "queued",
            python_path,
            runner_script_path
        ],
    )
    .map_err(|err| err.to_string())?;
    jobs::replace_segments_tx(&tx, job_id, "transcript", &[])?;
    jobs::replace_segments_tx(&tx, job_id, "speaker", &[])?;
    ai_summary_runs::delete_summary_runs_for_job_tx(&tx, job_id)?;
    insert_queued_job_run_tx(&tx, job_id)?;
    job_events::append_job_event_tx(&tx, job_id, "retried", "任务已重试。", None)?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn prepare_job_deletion(app: &AppHandle, job_id: &str) -> LocalResult<JobDeletionOperation> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| err.to_string())?;
    let operation_id = ids::timestamped_id("delete");
    let trash_name = format!("{job_id}-{operation_id}");
    let operation = job_deletions::prepare_tx(
        &tx,
        &operation_id,
        job_id,
        &trash_name,
        crate::infrastructure::time::unix_timestamp_millis() as u64,
    )?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(operation)
}

pub fn list_pending_job_deletions(app: &AppHandle) -> LocalResult<Vec<JobDeletionOperation>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    job_deletions::list(&conn)
}

pub fn job_exists(app: &AppHandle, job_id: &str) -> LocalResult<bool> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM jobs WHERE id = ?1)",
        params![job_id],
        |row| row.get::<_, bool>(0),
    )
    .map_err(|err| err.to_string())
}

pub fn mark_job_deletion_phase(
    app: &AppHandle,
    operation_id: &str,
    phase: JobDeletionPhase,
) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    job_deletions::set_phase(
        &conn,
        operation_id,
        phase,
        crate::infrastructure::time::unix_timestamp_millis() as u64,
    )
}

pub fn record_job_deletion_error(
    app: &AppHandle,
    operation_id: &str,
    error: &str,
) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    job_deletions::record_error(
        &conn,
        operation_id,
        error,
        crate::infrastructure::time::unix_timestamp_millis() as u64,
    )
}

pub fn delete_job_for_operation(
    app: &AppHandle,
    operation: &JobDeletionOperation,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| err.to_string())?;
    job_deletions::require(&tx, &operation.operation_id, &operation.job_id)?;
    ai_summary_runs::delete_summary_runs_for_job_tx(&tx, &operation.job_id)?;
    tx.execute(
        "DELETE FROM transcript_segments WHERE job_id = ?1",
        params![operation.job_id],
    )
    .map_err(|err| err.to_string())?;
    tx.execute(
        "DELETE FROM job_source_files WHERE job_id = ?1",
        params![operation.job_id],
    )
    .map_err(|err| err.to_string())?;
    tx.execute("DELETE FROM jobs WHERE id = ?1", params![operation.job_id])
        .map_err(|err| err.to_string())?;
    job_deletions::set_phase(
        &tx,
        &operation.operation_id,
        JobDeletionPhase::DatabaseDeleted,
        crate::infrastructure::time::unix_timestamp_millis() as u64,
    )?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn complete_job_deletion_operation(app: &AppHandle, operation_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    job_deletions::finish(&conn, operation_id)
}

pub fn list_ai_models(app: &AppHandle) -> LocalResult<Vec<AiModelMetadata>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_models::list_ai_models(&conn)
}

pub fn save_ai_model(app: &AppHandle, model: &AiModelSaveInput) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    crate::application::save_ai_model::save_ai_model(
        &mut conn,
        &crate::infrastructure::credentials::default_credential_store(),
        model,
    )
}

pub fn delete_ai_model(app: &AppHandle, model_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    crate::application::delete_ai_model::delete_ai_model(
        &mut conn,
        &crate::infrastructure::credentials::default_credential_store(),
        model_id,
    )
}

pub fn list_ai_templates(app: &AppHandle) -> LocalResult<Vec<AiSummaryTemplate>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_templates::list_ai_templates(&conn)
}

pub fn save_ai_template(app: &AppHandle, template: &AiSummaryTemplate) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_templates::save_ai_template(&conn, template)
}

pub fn delete_ai_template(app: &AppHandle, template_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_templates::delete_ai_template(&conn, template_id)
}

pub fn list_ai_summary_runs(app: &AppHandle, job_id: &str) -> LocalResult<Vec<AiSummaryRun>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_summary_runs::list_summary_runs(&conn, job_id)
}

pub fn list_meeting_members(app: &AppHandle) -> LocalResult<Vec<MeetingMember>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    members::list_meeting_members(&conn)
}

pub fn get_pet_profile(app: &AppHandle) -> LocalResult<PetProfile> {
    init_database(app)?;
    let conn = open_connection(app)?;
    pet::ensure_default_exists(&conn)?;
    pet::reconcile_profile_leveling(&conn)?;
    pet::load_profile(&conn)
}

pub fn get_pet_store_state(app: &AppHandle) -> LocalResult<PetStoreState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    pet::reconcile_profile_leveling(&conn)?;
    let profile = pet::load_profile(&conn)?;
    pet_store::store_state(&conn, profile)
}

pub fn get_pet_blind_box_state(app: &AppHandle) -> LocalResult<PetBlindBoxState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    pet::reconcile_profile_leveling(&conn)?;
    let profile = pet::load_profile(&conn)?;
    pet_blind_box::blind_box_state(&conn, profile)
}

pub fn draw_pet_blind_box(app: &AppHandle) -> LocalResult<PetBlindBoxDrawResult> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    let profile = pet::load_profile_tx(&tx)?;
    let draw = pet_blind_box::draw_blind_box_tx(&tx, &profile, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    let profile = pet::load_profile(&conn)?;
    pet_blind_box::draw_result(&conn, profile, draw)
}

pub fn get_pet_daily_check_in_state(app: &AppHandle) -> LocalResult<PetDailyCheckInState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    pet::reconcile_profile_leveling(&conn)?;
    let profile = pet::load_profile(&conn)?;
    pet_check_in::daily_check_in_state(&conn, profile)
}

pub fn claim_pet_daily_check_in(app: &AppHandle) -> LocalResult<PetDailyCheckInClaimResult> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    let profile = pet::load_profile_tx(&tx)?;
    let (entry, duplicate) = pet_check_in::claim_daily_check_in_tx(&tx, &profile, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    pet::reconcile_profile_leveling(&conn)?;
    let profile = pet::load_profile(&conn)?;
    pet_check_in::claim_result(&conn, profile, entry, duplicate)
}

pub fn repair_pet_daily_check_in(app: &AppHandle) -> LocalResult<PetDailyCheckInMakeupResult> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    let profile = pet::load_profile_tx(&tx)?;
    let entry = pet_check_in::repair_daily_check_in_tx(&tx, &profile, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    pet::reconcile_profile_leveling(&conn)?;
    let profile = pet::load_profile(&conn)?;
    pet_check_in::makeup_result(&conn, profile, entry)
}

pub fn save_pet_profile(app: &AppHandle, profile: &PetProfile) -> LocalResult<PetProfile> {
    init_database(app)?;
    let conn = open_connection(app)?;
    pet::ensure_default_exists(&conn)?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|err| err.to_string())?;
    pet::save_profile_tx(&tx, profile)?;
    tx.commit().map_err(|err| err.to_string())?;
    pet::load_profile(&conn)
}

pub fn get_pet_settings(app: &AppHandle) -> LocalResult<PetSettings> {
    init_database(app)?;
    let conn = open_connection(app)?;
    pet::ensure_default_exists(&conn)?;
    pet::load_settings(&conn)
}

pub fn save_pet_settings(app: &AppHandle, settings: &PetSettings) -> LocalResult<PetSettings> {
    init_database(app)?;
    let conn = open_connection(app)?;
    pet::ensure_default_exists(&conn)?;
    pet::save_settings(&conn, settings)?;
    pet::load_settings(&conn)
}

pub fn list_pet_event_ledger(
    app: &AppHandle,
    limit: usize,
) -> LocalResult<Vec<PetEventLedgerEntry>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    pet::ensure_default_exists(&conn)?;
    pet::list_event_ledger(&conn, limit)
}

pub fn list_pet_cosmetic_unlocks(app: &AppHandle) -> LocalResult<Vec<PetCosmeticUnlock>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    pet::ensure_default_exists(&conn)?;
    pet::list_cosmetic_unlocks(&conn)
}

pub fn apply_pet_growth_event(
    app: &AppHandle,
    event_type: &str,
    event_source: &str,
    event_value: i64,
    mood: &str,
    metadata: Option<&str>,
) -> LocalResult<PetProfile> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    pet_growth::apply_pet_growth_event(
        &mut conn,
        event_type,
        event_source,
        event_value,
        mood,
        metadata,
    )
}

pub fn purchase_pet_store_item(
    app: &AppHandle,
    item_key: &str,
    quantity: i64,
) -> LocalResult<PetStoreState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    let profile = pet::load_profile_tx(&tx)?;
    pet_store::purchase_item_tx(&tx, &profile, item_key, quantity, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    let profile = pet::load_profile(&conn)?;
    pet_store::store_state(&conn, profile)
}

pub fn equip_pet_inventory_item(app: &AppHandle, item_key: &str) -> LocalResult<PetStoreState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    pet_store::equip_item_tx(&tx, item_key, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    let profile = pet::load_profile(&conn)?;
    pet_store::store_state(&conn, profile)
}

pub fn unequip_pet_inventory_slot(app: &AppHandle, slot: &str) -> LocalResult<PetStoreState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    pet_store::unequip_slot_tx(&tx, slot, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    let profile = pet::load_profile(&conn)?;
    pet_store::store_state(&conn, profile)
}

pub fn use_pet_inventory_item(
    app: &AppHandle,
    item_key: &str,
    quantity: i64,
) -> LocalResult<PetStoreState> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    pet_store::use_item_tx(&tx, item_key, quantity, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    let profile = pet::load_profile(&conn)?;
    pet_store::store_state(&conn, profile)
}

pub fn open_pet_gift_box(app: &AppHandle) -> LocalResult<PetGiftBoxOpenResult> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    let (prize, duplicate, duplicate_compensation_lp) = pet_store::open_gift_box_tx(&tx, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    let profile = pet::load_profile(&conn)?;
    Ok(PetGiftBoxOpenResult {
        state: pet_store::store_state(&conn, profile)?,
        prize,
        duplicate,
        duplicate_compensation_lp,
    })
}

pub fn redeem_pet_key(app: &AppHandle, key: &str) -> LocalResult<PetRedeemKeyResult> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    let profile = pet::load_profile_tx(&tx)?;
    let (redemption, rewards, duplicate) = pet_redeem_key::redeem_key_tx(&tx, profile, key, &now)?;
    tx.commit().map_err(|err| err.to_string())?;
    pet::reconcile_profile_leveling(&conn)?;
    let profile = pet::load_profile(&conn)?;
    Ok(PetRedeemKeyResult {
        state: pet_store::store_state(&conn, profile)?,
        redemption,
        rewards,
        duplicate,
    })
}

pub fn list_pet_redeem_key_redemptions(
    app: &AppHandle,
    limit: usize,
) -> LocalResult<Vec<PetRedeemKeyRedemption>> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    let values = pet_redeem_key::list_redemptions_tx(&tx, limit.clamp(1, 100))?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(values)
}

pub fn save_meeting_member(app: &AppHandle, member: &MeetingMember) -> LocalResult<()> {
    init_database(app)?;

    if member.name.trim().is_empty() {
        return Err("姓名不能为空。".into());
    }

    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    members::save_meeting_member_tx(&tx, member)?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn delete_meeting_member(app: &AppHandle, member_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    members::delete_meeting_member(&conn, member_id)
}

pub fn import_meeting_members(
    app: &AppHandle,
    members: &[MeetingMember],
) -> LocalResult<MeetingMemberImportResult> {
    init_database(app)?;

    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let mut stmt = tx
        .prepare("SELECT id, name, created_at FROM meeting_members")
        .map_err(|err| err.to_string())?;

    let existing_rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    drop(stmt);

    let mut existing_by_name = std::collections::HashMap::new();
    for (id, name, created_at) in existing_rows {
        existing_by_name.insert(name.trim().to_string(), (id, created_at));
    }

    let mut created = 0usize;
    let mut updated = 0usize;

    for (index, member) in members.iter().enumerate() {
        let normalized_name = member.name.trim().to_string();
        let (id, created_at, is_update) = match existing_by_name.get(&normalized_name) {
            Some((existing_id, existing_created_at)) => {
                (existing_id.clone(), existing_created_at.clone(), true)
            }
            None => (
                ids::timestamped_indexed_id("member", index),
                member.created_at.clone(),
                false,
            ),
        };

        let next_member = MeetingMember {
            id,
            name: normalized_name.clone(),
            department: member.department.trim().to_string(),
            sort_order: member.sort_order,
            is_recorder: member.is_recorder,
            created_at,
            updated_at: member.updated_at.clone(),
        };

        members::save_meeting_member_tx(&tx, &next_member)?;
        existing_by_name.insert(
            normalized_name,
            (next_member.id.clone(), next_member.created_at.clone()),
        );

        if is_update {
            updated += 1;
        } else {
            created += 1;
        }
    }

    tx.commit().map_err(|err| err.to_string())?;

    Ok(MeetingMemberImportResult { created, updated })
}

pub fn set_active_ai_summary_run(app: &AppHandle, job_id: &str, run_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_summary_runs::set_active_summary_run(&conn, job_id, run_id)
}

pub fn delete_ai_summary_run(app: &AppHandle, job_id: &str, run_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    ai_summary_runs::delete_summary_run(&mut conn, job_id, run_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_reset_commits_results_and_persistent_queue_together() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE jobs (
               id TEXT PRIMARY KEY,
               upload_status TEXT NOT NULL,
               asr_status TEXT NOT NULL,
               summary_status TEXT NOT NULL,
               overall_status TEXT NOT NULL,
               failure_reason TEXT,
               process_log TEXT,
               duration_minutes INTEGER NOT NULL,
               processing_started_at_ms INTEGER,
               processing_finished_at_ms INTEGER,
               processing_duration_seconds INTEGER,
               python_path TEXT,
               runner_script_path TEXT,
               enable_speaker INTEGER NOT NULL DEFAULT 1,
               runner_protocol_version INTEGER,
               asr_backend TEXT NOT NULL DEFAULT 'unknown',
               diarization_status TEXT NOT NULL DEFAULT 'pending',
               warnings_json TEXT NOT NULL DEFAULT '[]'
             );
             CREATE TABLE transcript_segments (job_id TEXT, segment_type TEXT);
             CREATE TABLE ai_summary_runs (id TEXT PRIMARY KEY, job_id TEXT NOT NULL);
             CREATE TABLE job_runs (
               job_id TEXT PRIMARY KEY,
               attempt_id INTEGER NOT NULL,
               lease_token INTEGER NOT NULL,
               status TEXT NOT NULL,
               pid INTEGER,
               process_identity TEXT,
               heartbeat_at_ms INTEGER,
               started_at_ms INTEGER,
               finished_at_ms INTEGER,
               output_dir TEXT,
               last_error TEXT
             );
             CREATE TABLE job_events (
               id TEXT PRIMARY KEY,
               job_id TEXT NOT NULL,
               event_type TEXT NOT NULL,
               message TEXT NOT NULL,
               metadata_json TEXT,
               created_at TEXT NOT NULL
             );
             INSERT INTO jobs(
               id, upload_status, asr_status, summary_status, overall_status,
               failure_reason, process_log, duration_minutes,
               processing_started_at_ms, processing_finished_at_ms,
               processing_duration_seconds, python_path, runner_script_path
             ) VALUES(
               'job-1700000000000-1', 'uploaded', 'failed', 'completed', 'failed',
               'old failure', 'old log', 12, 10, 20, 10, '/old/python', '/old/runner'
             );
             INSERT INTO transcript_segments VALUES
               ('job-1700000000000-1', 'transcript'),
               ('job-1700000000000-1', 'speaker');
             INSERT INTO ai_summary_runs VALUES('summary-1', 'job-1700000000000-1');
             INSERT INTO job_runs VALUES(
               'job-1700000000000-1', 4, 9, 'failed', 77, 'old identity', 20,
               10, 20, 'attempts/attempt-4-9', 'old failure'
             );",
        )
        .unwrap();

        reset_job_for_retry_on_connection(
            &mut conn,
            "job-1700000000000-1",
            "/new/python",
            "/new/runner",
        )
        .unwrap();

        let job_state = conn
            .query_row(
                "SELECT asr_status, summary_status, overall_status, failure_reason,
                        process_log, duration_minutes, processing_started_at_ms,
                        python_path, runner_script_path
                 FROM jobs WHERE id = 'job-1700000000000-1'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(job_state.0, "queued");
        assert_eq!(job_state.1, "idle");
        assert_eq!(job_state.2, "queued");
        assert_eq!(job_state.3, None);
        assert_eq!(job_state.4, None);
        assert_eq!(job_state.5, 0);
        assert_eq!(job_state.6, None);
        assert_eq!(job_state.7, "/new/python");
        assert_eq!(job_state.8, "/new/runner");

        let run_state = conn
            .query_row(
                "SELECT attempt_id, lease_token, status, pid, process_identity, output_dir
                 FROM job_runs WHERE job_id = 'job-1700000000000-1'",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(run_state, (4, 9, "queued".into(), None, None, None));
        let remaining_results: i64 = conn
            .query_row("SELECT COUNT(*) FROM transcript_segments", [], |row| {
                row.get(0)
            })
            .unwrap();
        let remaining_summaries: i64 = conn
            .query_row("SELECT COUNT(*) FROM ai_summary_runs", [], |row| row.get(0))
            .unwrap();
        let retry_events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM job_events WHERE event_type = 'retried'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            (remaining_results, remaining_summaries, retry_events),
            (0, 0, 1)
        );
    }
}
