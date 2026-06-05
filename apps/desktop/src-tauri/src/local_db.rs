use crate::infrastructure::{
    ids, migrations,
    repositories::{
        ai_models, ai_summary_runs, ai_templates, job_events, members, pet, pet_blind_box,
        pet_check_in, pet_redeem_key, pet_store, runtime_state, settings,
    },
};
use rusqlite::{params, Connection};
use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tauri::{AppHandle, Manager};

pub type LocalResult<T> = Result<T, String>;

const MAX_DAILY_INTERACTION_PER_SOURCE: i64 = 20;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

static INIT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub use model::*;

mod jobs;
mod legacy;
mod model;
mod pet_growth;
pub(crate) mod pet_leveling;
mod progress;
mod schema;

pub fn init_database(app: &AppHandle) -> LocalResult<()> {
    let _guard = INIT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|err| format!("本地数据库初始化锁异常: {err}"))?;
    let mut conn = open_connection(app)?;
    schema::apply_schema(&conn)?;
    migrations::ensure_schema_version(&conn)?;
    schema::seed_builtin_templates(&conn)?;
    legacy::import_legacy_jobs(app, &mut conn)?;
    Ok(())
}

pub fn open_connection(app: &AppHandle) -> LocalResult<Connection> {
    let path = database_path(app)?;
    let conn = Connection::open(&path)
        .map_err(|err| format!("无法打开本地数据库 {}: {err}", path.display()))?;
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)
        .map_err(|err| format!("无法设置本地数据库等待超时 {}: {err}", path.display()))?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
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

pub fn job_dir(app: &AppHandle, job_id: &str) -> LocalResult<PathBuf> {
    Ok(jobs_root(app)?.join(job_id))
}

pub fn list_jobs(app: &AppHandle) -> LocalResult<Vec<MeetingJob>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id FROM jobs
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

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    settings::save_settings(&conn, settings)
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

pub fn save_job_snapshot(app: &AppHandle, job: &MeetingJob) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    jobs::save_job_snapshot_tx(&tx, job)?;
    job_events::append_job_event_tx(&tx, &job.id, "created", "任务已创建。", None)?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn update_job_statuses(
    app: &AppHandle,
    job_id: &str,
    asr_status: &str,
    summary_status: &str,
    overall_status: &str,
    failure_reason: Option<&str>,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE jobs
         SET asr_status = ?2,
             summary_status = ?3,
             overall_status = ?4,
             failure_reason = ?5
         WHERE id = ?1",
        params![
            job_id,
            asr_status,
            summary_status,
            overall_status,
            failure_reason
        ],
    )
    .map_err(|err| err.to_string())?;
    job_events::append_job_event_tx(
        &tx,
        job_id,
        overall_status,
        failure_reason.unwrap_or(""),
        None,
    )?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn update_job_process_log(app: &AppHandle, job_id: &str, process_log: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE jobs SET process_log = ?2 WHERE id = ?1",
        params![job_id, process_log],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn replace_job_segments(
    app: &AppHandle,
    job_id: &str,
    transcript_segments: &[TranscriptSegment],
    speaker_segments: &[TranscriptSegment],
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    jobs::replace_segments_tx(&tx, job_id, "transcript", transcript_segments)?;
    jobs::replace_segments_tx(&tx, job_id, "speaker", speaker_segments)?;
    tx.commit().map_err(|err| err.to_string())
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

pub fn update_job_completion(
    app: &AppHandle,
    job_id: &str,
    duration_minutes: u32,
    processing_finished_at_ms: u64,
    processing_duration_seconds: Option<u32>,
    failure_reason: Option<&str>,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE jobs
         SET duration_minutes = ?2,
             processing_finished_at_ms = ?3,
             processing_duration_seconds = ?4,
             summary_status = ?5,
             asr_status = ?6,
             overall_status = ?7,
             failure_reason = ?8
         WHERE id = ?1",
        params![
            job_id,
            duration_minutes,
            processing_finished_at_ms as i64,
            processing_duration_seconds.map(i64::from),
            "idle",
            "completed",
            "completed",
            failure_reason
        ],
    )
    .map_err(|err| err.to_string())?;
    job_events::append_job_event_tx(&tx, job_id, "completed", "任务处理完成。", None)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn mark_job_processing_started(
    app: &AppHandle,
    job_id: &str,
    processing_started_at_ms: u64,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE jobs
         SET processing_started_at_ms = ?2,
             processing_finished_at_ms = NULL,
             processing_duration_seconds = NULL
         WHERE id = ?1",
        params![job_id, processing_started_at_ms as i64],
    )
    .map_err(|err| err.to_string())?;
    job_events::append_job_event_tx(&tx, job_id, "started", "任务开始处理。", None)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn update_job_failure(
    app: &AppHandle,
    job_id: &str,
    processing_finished_at_ms: u64,
    processing_duration_seconds: Option<u32>,
    failure_reason: &str,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE jobs
         SET processing_finished_at_ms = ?2,
             processing_duration_seconds = ?3,
             asr_status = 'failed',
             summary_status = 'idle',
             overall_status = 'failed',
             failure_reason = ?4
         WHERE id = ?1",
        params![
            job_id,
            processing_finished_at_ms as i64,
            processing_duration_seconds.map(i64::from),
            failure_reason
        ],
    )
    .map_err(|err| err.to_string())?;
    job_events::append_job_event_tx(&tx, job_id, "failed", failure_reason, None)?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn reset_job_for_retry(
    app: &AppHandle,
    job_id: &str,
    python_path: &str,
    runner_script_path: &str,
) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE jobs
         SET upload_status = ?2,
             asr_status = ?3,
             summary_status = ?4,
             overall_status = ?5,
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
    job_events::append_job_event_tx(&tx, job_id, "retried", "任务已重试。", None)?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn delete_job(app: &AppHandle, job_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    ai_summary_runs::delete_summary_runs_for_job_tx(&tx, job_id)?;
    tx.execute(
        "DELETE FROM transcript_segments WHERE job_id = ?1",
        params![job_id],
    )
    .map_err(|err| err.to_string())?;
    tx.execute(
        "DELETE FROM job_source_files WHERE job_id = ?1",
        params![job_id],
    )
    .map_err(|err| err.to_string())?;
    tx.execute("DELETE FROM jobs WHERE id = ?1", params![job_id])
        .map_err(|err| err.to_string())?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn list_ai_models(app: &AppHandle) -> LocalResult<Vec<AiModelConfig>> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_models::list_ai_models(&conn)
}

pub fn save_ai_model(app: &AppHandle, model: &AiModelConfig) -> LocalResult<()> {
    init_database(app)?;
    let mut conn = open_connection(app)?;
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    ai_models::save_ai_model_tx(&tx, model)?;
    tx.commit().map_err(|err| err.to_string())
}

pub fn delete_ai_model(app: &AppHandle, model_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_models::delete_ai_model(&conn, model_id)
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

pub fn save_ai_summary_run(app: &AppHandle, run: &AiSummaryRun) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_summary_runs::save_summary_run(&conn, run)?;
    ai_summary_runs::update_job_summary_status_after_save(&conn, run)
}

pub fn set_active_ai_summary_run(app: &AppHandle, job_id: &str, run_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE jobs SET active_summary_run_id = ?2 WHERE id = ?1",
        params![job_id, run_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn delete_ai_summary_run(app: &AppHandle, job_id: &str, run_id: &str) -> LocalResult<()> {
    init_database(app)?;
    let conn = open_connection(app)?;
    ai_summary_runs::delete_summary_run(&conn, job_id, run_id)?;

    let remaining_runs = ai_summary_runs::list_summary_runs(&conn, job_id)?;
    let next_active_run = remaining_runs
        .iter()
        .find(|run| run.status == "completed" && run.result.is_some())
        .cloned()
        .or_else(|| remaining_runs.first().cloned());
    let summary_status = if remaining_runs.iter().any(|run| run.status == "running") {
        "summarizing"
    } else if next_active_run
        .as_ref()
        .and_then(|run| run.result.as_ref())
        .is_some()
    {
        "completed"
    } else if remaining_runs.iter().any(|run| run.status == "failed") {
        "failed"
    } else {
        "idle"
    };

    ai_summary_runs::update_job_summary_selection(
        &conn,
        job_id,
        summary_status,
        next_active_run.as_ref().map(|run| run.id.clone()),
    )
}
