use crate::infrastructure::process_logs;
use crate::infrastructure::runner_files;
use crate::infrastructure::runner_process::{self, RunnerCommandInput, RunnerExit};
use crate::infrastructure::time::unix_timestamp_millis;
use crate::local_db::{self, MeetingJob, MeetingSourceFile, MeetingSummary, TranscriptSegment};
use crate::local_runtime;
use serde::Deserialize;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, Webview, WebviewWindow};
use tauri_plugin_fs::FsExt;

#[path = "infrastructure/job_scheduler.rs"]
mod job_scheduler;

use job_scheduler::{
    JobExecution, JobExecutionError, JobExecutionMode, JobExecutor, JobRunCompletion,
    JobRunContext, JobRunRegistration, JobRunStore, JobScheduler, PersistedJobState,
    RecoverableJob, RunFence, SchedulerResult,
};

static JOB_COUNTER: AtomicU64 = AtomicU64::new(1);
static JOB_SCHEDULER: OnceLock<JobScheduler> = OnceLock::new();
const DELETE_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const DATABASE_PROCESS_LOG_BYTES: usize = 64 * 1024;

type LocalResult<T> = Result<T, String>;

struct LocalJobRunStore {
    app: AppHandle,
}

impl JobRunStore for LocalJobRunStore {
    fn load_recoverable_jobs(&self) -> SchedulerResult<Vec<RecoverableJob>> {
        local_db::list_recoverable_local_job_runs(&self.app).map(|runs| {
            runs.into_iter()
                .map(|run| RecoverableJob {
                    job_id: run.job_id,
                    state: if run.status == "queued" {
                        PersistedJobState::Queued
                    } else {
                        PersistedJobState::Running
                    },
                    fence: Some(RunFence {
                        attempt_id: run.attempt_id,
                        lease_token: run.lease_token,
                    }),
                    pid: run.pid,
                    process_identity: run.process_identity,
                    started_at_ms: run.started_at_ms,
                    heartbeat_at_ms: run.heartbeat_at_ms,
                })
                .collect()
        })
    }

    fn requeue_recovered_job(
        &self,
        job_id: &str,
        previous_fence: Option<RunFence>,
    ) -> SchedulerResult<()> {
        let fence = previous_fence.ok_or_else(|| "恢复中的任务缺少运行 fence。".to_string())?;
        if local_db::requeue_recovered_local_job_run(
            &self.app,
            job_id,
            fence.attempt_id,
            fence.lease_token,
        )? {
            Ok(())
        } else {
            Err("恢复任务时运行 fence 已失效。".into())
        }
    }

    fn begin_run(&self, registration: &JobRunRegistration) -> SchedulerResult<RunFence> {
        let lease = local_db::begin_local_job_run(
            &self.app,
            &registration.job_id,
            registration.started_at_ms,
        )?;
        Ok(RunFence {
            attempt_id: lease.attempt_id,
            lease_token: lease.lease_token,
        })
    }

    fn fence_job(&self, job_id: &str) -> SchedulerResult<()> {
        local_db::fence_local_job_run(&self.app, job_id)
    }

    fn attach_process(
        &self,
        job_id: &str,
        fence: RunFence,
        pid: u32,
        process_identity: &str,
        heartbeat_at_ms: u64,
    ) -> SchedulerResult<bool> {
        local_db::attach_local_job_process(
            &self.app,
            job_id,
            fence.attempt_id,
            fence.lease_token,
            pid,
            process_identity,
            heartbeat_at_ms,
        )
    }

    fn heartbeat(
        &self,
        job_id: &str,
        fence: RunFence,
        pid: Option<u32>,
        heartbeat_at_ms: u64,
    ) -> SchedulerResult<bool> {
        local_db::heartbeat_local_job_run(
            &self.app,
            job_id,
            fence.attempt_id,
            fence.lease_token,
            pid,
            heartbeat_at_ms,
        )
    }

    fn is_current_fence(&self, job_id: &str, fence: RunFence) -> SchedulerResult<bool> {
        local_db::is_current_local_job_run(&self.app, job_id, fence.attempt_id, fence.lease_token)
    }

    fn finish_run(
        &self,
        job_id: &str,
        fence: RunFence,
        completion: &JobRunCompletion,
    ) -> SchedulerResult<bool> {
        match completion {
            JobRunCompletion::Completed => local_db::local_job_run_has_status(
                &self.app,
                job_id,
                fence.attempt_id,
                fence.lease_token,
                "completed",
            ),
            JobRunCompletion::Failed { reason } => {
                if local_db::local_job_run_has_status(
                    &self.app,
                    job_id,
                    fence.attempt_id,
                    fence.lease_token,
                    "failed",
                )? {
                    return Ok(true);
                }
                let execution = JobExecution {
                    job_id: job_id.to_string(),
                    fence,
                    started_at_ms: 0,
                    mode: JobExecutionMode::Fresh,
                };
                mark_failed(&self.app, &execution, reason)
            }
            JobRunCompletion::Cancelled => local_db::requeue_recovered_local_job_run(
                &self.app,
                job_id,
                fence.attempt_id,
                fence.lease_token,
            ),
        }
    }
}

struct LocalJobExecutor {
    app: AppHandle,
}

impl JobExecutor for LocalJobExecutor {
    fn execute(
        &self,
        execution: JobExecution,
        context: JobRunContext,
    ) -> Result<(), JobExecutionError> {
        match execute_local_job(&self.app, &execution, &context) {
            Err(JobExecutionError::Failed(reason)) => {
                match mark_failed(&self.app, &execution, &reason) {
                    Ok(true) => Err(JobExecutionError::Failed(reason)),
                    Ok(false) => Err(JobExecutionError::Cancelled),
                    Err(error) => Err(JobExecutionError::Failed(error)),
                }
            }
            result => result,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobInput {
    pub title: String,
    pub files: Vec<MeetingSourceFile>,
    pub hotwords: Vec<String>,
    pub lang: String,
    pub enable_speaker: bool,
    pub summary_template: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunnerResult {
    duration_minutes: Option<u32>,
    transcript_segments: Option<Vec<TranscriptSegment>>,
    speaker_segments: Option<Vec<TranscriptSegment>>,
    failure_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressSnapshot {
    stage: String,
    status_message: Option<String>,
    failure_reason: Option<String>,
}

#[tauri::command]
pub fn list_jobs(app: AppHandle) -> LocalResult<Vec<MeetingJob>> {
    local_db::list_jobs(&app)
}

#[tauri::command]
pub fn get_job(app: AppHandle, id: String) -> LocalResult<MeetingJob> {
    runner_files::validate_job_id(&id)?;
    local_db::get_job(&app, &id)
}

#[tauri::command]
pub fn get_job_result(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
    window_scope_token: Option<String>,
) -> LocalResult<MeetingJob> {
    runner_files::validate_job_id(&id)?;
    crate::window_scope::authorize_job_window(
        &window,
        &[
            crate::window_scope::ai_summary_window(),
            crate::window_scope::meeting_notes_window(),
        ],
        &id,
        window_scope_token.as_deref(),
    )?;
    local_db::get_job(&app, &id)
}

#[tauri::command]
pub fn rename_job_speaker(
    app: AppHandle,
    id: String,
    from_speaker: String,
    to_speaker: String,
) -> LocalResult<MeetingJob> {
    runner_files::validate_job_id(&id)?;
    local_db::rename_job_speaker(&app, &id, &from_speaker, &to_speaker)?;
    local_db::get_job(&app, &id)
}

#[tauri::command]
pub fn delete_job(app: AppHandle, id: String) -> LocalResult<()> {
    runner_files::validate_job_id(&id)?;
    local_db::get_job(&app, &id)?;
    let scheduler = job_scheduler(&app)?;
    let mut deletion = scheduler.reserve_deletion(id.clone())?;
    let operation = local_db::prepare_job_deletion(&app, &id)?;
    deletion.persist_intent();
    let result = (|| {
        deletion.fence_and_cancel()?;
        deletion.wait_until_idle(DELETE_WAIT_TIMEOUT)?;
        converge_job_deletion(&app, &operation)
    })();
    if let Err(error) = result {
        let _ = local_db::record_job_deletion_error(&app, &operation.operation_id, &error);
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub fn create_job(
    app: AppHandle,
    webview: Webview,
    input: CreateJobInput,
) -> LocalResult<MeetingJob> {
    validate_create_input(&app, &input)?;
    for file in &input.files {
        let path = file
            .path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .ok_or_else(|| "本地任务文件缺少路径。".to_string())?;
        if !webview.fs_scope().is_allowed(Path::new(path)) {
            return Err("任务文件未经系统选择对话框授权。".into());
        }
    }
    let scheduler = job_scheduler(&app)?;
    let runner_script_path = resolve_runner_script_path(&app)?;
    let job = build_initial_job(input, runner_script_path);
    let dir = safe_job_dir(&app, &job.id)?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    local_db::save_job_snapshot(&app, &job)?;
    let queued_snapshot = local_db::get_job(&app, &job.id)?;
    scheduler.enqueue(job.id.clone())?;
    Ok(queued_snapshot)
}

#[tauri::command]
pub fn retry_job(app: AppHandle, id: String) -> LocalResult<MeetingJob> {
    runner_files::validate_job_id(&id)?;
    let scheduler = job_scheduler(&app)?;
    let reservation = scheduler.reserve_job(id.clone())?;
    let job = local_db::get_job(&app, &id)?;
    let first_file = job
        .source_files
        .first()
        .ok_or_else(|| "任务缺少输入文件。".to_string())?;

    if first_file.path.as_deref().unwrap_or("").trim().is_empty() {
        return Err("本地模式只支持带本地路径的文件。".into());
    }

    let dir = safe_job_dir(&app, &id)?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let runner_script_path = resolve_runner_script_path(&app)?;
    local_db::reset_job_for_retry(&app, &id, "", &runner_script_path)?;
    let queued_snapshot = local_db::get_job(&app, &id)?;
    reservation.enqueue()?;
    Ok(queued_snapshot)
}

fn execute_local_job(
    app: &AppHandle,
    execution: &JobExecution,
    context: &JobRunContext,
) -> Result<(), JobExecutionError> {
    let job_id = execution.job_id.as_str();
    runner_files::validate_job_id(job_id)?;
    context.ensure_current()?;
    let job_dir = safe_job_dir(app, job_id)?;
    let dir = runner_files::create_attempt_dir(
        &job_dir,
        execution.fence.attempt_id,
        execution.fence.lease_token,
    )?;
    if execution.mode == JobExecutionMode::Fresh {
        process_logs::reset(&dir)?;
        reset_runner_files(&dir)?;
    }
    let job = local_db::get_job(app, job_id)?;
    let settings = local_db::get_settings(app)?;
    let input_file = job
        .source_files
        .first()
        .and_then(|file| file.path.clone())
        .ok_or_else(|| "任务缺少可处理的本地文件路径。".to_string())?;
    let resolved_runtime = local_runtime::resolve_python_runtime(app, &settings)?;
    let runner_script_path = resolve_runner_script_path(app)?;

    let processing_started_at_ms = unix_timestamp_millis() as u64;
    let runtime_threads = runner_process::resolve_local_asr_threads(&settings);
    append_process_log_line(
        &dir,
        &format!(
            "[runner] source={}, backend={}, device={}, threads={}, batch_size_s={}, speaker={}, ffmpeg={}",
            resolved_runtime.source_label,
            resolved_runtime.asr_backend,
            runner_process::normalize_local_asr_device(&settings),
            runtime_threads,
            settings.local_asr_batch_size_seconds,
            if job.enable_speaker { "true" } else { "false" },
            resolved_runtime
                .ffmpeg_path
                .as_deref()
                .unwrap_or("not-configured")
        ),
    )?;

    runner_process::validate_runtime_tools(
        &dir,
        &input_file,
        resolved_runtime.ffmpeg_path.as_deref(),
    )?;
    sync_process_log(app, execution, &dir)?;

    let mut command = runner_process::build_runner_command(RunnerCommandInput {
        runtime: &resolved_runtime,
        settings: &settings,
        job: &job,
        job_dir: &dir,
        runner_script_path: Path::new(&runner_script_path),
        input_file: Path::new(&input_file),
        runtime_threads,
    });

    runner_process::configure_runner_process(&mut command);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("无法启动本地 Python 处理进程: {err}"))?;
    let process_identity = match runner_process::capture_runner_process_identity(child.id()) {
        Ok(identity) => identity,
        Err(error) => {
            return match runner_process::terminate_runner(&mut child) {
                Ok(()) => Err(error.into()),
                Err(termination_error) => Err(format!("{error}; {termination_error}").into()),
            };
        }
    };
    if let Err(error) = context.register_process(child.id(), &process_identity) {
        return match runner_process::terminate_runner(&mut child) {
            Ok(()) => Err(error.into()),
            Err(termination_error) => Err(format!("{error}; {termination_error}").into()),
        };
    }
    let runner_exit = runner_process::stream_child_logs(
        app,
        job_id,
        &dir,
        &mut child,
        |app, _, dir| sync_process_log(app, execution, dir),
        || context.is_cancelled(),
        |_| context.heartbeat(),
    )?;
    let status = match runner_exit {
        RunnerExit::Finished(status) => status,
        RunnerExit::Cancelled => return Err(JobExecutionError::Cancelled),
    };

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        let detailed = read_runner_failure_reason(&dir)
            .or_else(|| process_logs::summarize_tail(&dir, 12))
            .unwrap_or_else(|| format!("本地 Python 处理失败，退出码 {code}。"));
        return Err(JobExecutionError::Failed(detailed));
    }

    let runner_result = read_runner_result(&dir)?;
    if let Some(reason) = runner_result.failure_reason.clone() {
        return Err(JobExecutionError::Failed(reason));
    }

    let transcript_segments = runner_result.transcript_segments.unwrap_or_default();
    let speaker_segments = runner_result
        .speaker_segments
        .unwrap_or_else(|| transcript_segments.clone());

    if job.enable_speaker && speaker_segments.is_empty() {
        return Err("Runner 未返回说话人分离结果。".into());
    }

    let processing_finished_at_ms = unix_timestamp_millis() as u64;
    let processing_duration_seconds =
        Some(((processing_finished_at_ms.saturating_sub(processing_started_at_ms)) / 1000) as u32);
    let process_log = process_logs::read_recent(&dir, DATABASE_PROCESS_LOG_BYTES);
    let accepted = local_db::complete_local_job_run(
        app,
        job_id,
        execution.fence.attempt_id,
        execution.fence.lease_token,
        &transcript_segments,
        &speaker_segments,
        derived_duration_minutes(
            runner_result.duration_minutes,
            job.duration_minutes,
            &transcript_segments,
            &speaker_segments,
        ),
        processing_finished_at_ms,
        processing_duration_seconds,
        &process_log,
    )?;
    if !accepted {
        return Err(JobExecutionError::Cancelled);
    }

    Ok(())
}

fn build_initial_job(input: CreateJobInput, runner_script_path: String) -> MeetingJob {
    MeetingJob {
        id: make_job_id(),
        source: "local".into(),
        title: input.title,
        source_files: input.files,
        duration_minutes: 0,
        processing_started_at_ms: None,
        processing_finished_at_ms: None,
        processing_duration_seconds: None,
        progress_percent: Some(0),
        progress_message: Some("等待开始处理。".into()),
        created_at: input.created_at,
        hotwords: input.hotwords,
        lang: input.lang,
        enable_speaker: input.enable_speaker,
        summary_template: input.summary_template,
        upload_status: "uploaded".into(),
        asr_status: "queued".into(),
        summary_status: "idle".into(),
        overall_status: "queued".into(),
        failure_reason: None,
        transcript_segments: Vec::new(),
        speaker_segments: Vec::new(),
        summary: empty_summary(),
        summary_runs: Vec::new(),
        active_summary_run_id: None,
        export_formats: vec!["txt".into(), "md".into(), "srt".into(), "docx".into()],
        last_exported_at: None,
        process_log: None,
        python_path: None,
        runner_script_path: Some(runner_script_path),
    }
}

fn validate_create_input(_app: &AppHandle, input: &CreateJobInput) -> LocalResult<()> {
    if input.title.trim().is_empty() {
        return Err("任务标题不能为空。".into());
    }

    if input.files.len() != 1 {
        return Err("本地 FunASR 模式当前只支持单文件任务。".into());
    }

    let file = input
        .files
        .first()
        .ok_or_else(|| "请选择一个输入文件。".to_string())?;
    let file_path = file
        .path
        .as_deref()
        .ok_or_else(|| "本地模式只支持带本地路径的文件。".to_string())?;

    if !Path::new(file_path).exists() {
        return Err("输入文件不存在或当前路径不可访问。".into());
    }

    Ok(())
}

fn resolve_runner_script_path(app: &AppHandle) -> LocalResult<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join("runner.py"));
        candidates.push(resource_dir.join("runner.py"));
        candidates.push(resource_dir.join("_up_").join("scripts").join("runner.py"));
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("../../../python/funasr-runner/runner.py"));

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("../../python/funasr-runner/runner.py"));
        candidates.push(current_dir.join("python/funasr-runner/runner.py"));
    }

    if let Ok(executable_path) = std::env::current_exe() {
        if let Some(executable_dir) = executable_path.parent() {
            candidates.push(executable_dir.join("scripts/runner.py"));
            candidates.push(executable_dir.join("../Resources/scripts/runner.py"));
            candidates.push(executable_dir.join("../Resources/runner.py"));
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            let resolved = candidate.canonicalize().unwrap_or(candidate);
            return Ok(resolved.to_string_lossy().into_owned());
        }
    }

    Err("未找到内置 Runner 脚本，请检查应用资源或 python/funasr-runner 目录。".into())
}

fn append_process_log_line(job_dir: &Path, line: &str) -> LocalResult<()> {
    process_logs::append_line(job_dir, line)
}

fn reset_runner_files(job_dir: &Path) -> LocalResult<()> {
    runner_files::reset_runner_files(job_dir)
}

fn sync_process_log(app: &AppHandle, execution: &JobExecution, job_dir: &Path) -> LocalResult<()> {
    let log = process_logs::read_recent(job_dir, DATABASE_PROCESS_LOG_BYTES);
    if local_db::update_local_job_process_log(
        app,
        &execution.job_id,
        execution.fence.attempt_id,
        execution.fence.lease_token,
        &log,
    )? {
        Ok(())
    } else {
        Err("任务运行 fence 已失效，拒绝发布进程日志。".into())
    }
}

fn mark_failed(app: &AppHandle, execution: &JobExecution, reason: &str) -> LocalResult<bool> {
    let job_id = execution.job_id.as_str();
    let dir = runner_files::resolve_attempt_dir(
        &safe_job_dir(app, job_id)?,
        execution.fence.attempt_id,
        execution.fence.lease_token,
    )?
    .ok_or_else(|| "任务 attempt 目录不存在。".to_string())?;
    let detailed_reason = read_runner_failure_reason(&dir)
        .or_else(|| process_logs::summarize_tail(&dir, 12))
        .unwrap_or_else(|| reason.to_string());
    let job = local_db::get_job(app, job_id)?;
    let processing_finished_at_ms = unix_timestamp_millis() as u64;
    let processing_duration_seconds = job
        .processing_started_at_ms
        .map(|started_at| ((processing_finished_at_ms.saturating_sub(started_at)) / 1000) as u32);
    let process_log = process_logs::read_recent(&dir, DATABASE_PROCESS_LOG_BYTES);
    local_db::fail_local_job_run(
        app,
        job_id,
        execution.fence.attempt_id,
        execution.fence.lease_token,
        processing_finished_at_ms,
        processing_duration_seconds,
        &detailed_reason,
        &process_log,
    )
}

fn job_scheduler(app: &AppHandle) -> LocalResult<&'static JobScheduler> {
    let settings = local_db::get_settings(app)?;
    let scheduler = JOB_SCHEDULER.get_or_init(|| {
        JobScheduler::new(
            settings.concurrency as usize,
            Arc::new(LocalJobRunStore { app: app.clone() }),
            Arc::new(LocalJobExecutor { app: app.clone() }),
            Arc::new(|job: &RecoverableJob| {
                runner_process::terminate_persisted_runner(job.pid, job.process_identity.as_deref())
            }),
        )
    });
    scheduler.set_max_concurrency(settings.concurrency as usize)?;
    scheduler.start_recovery()?;
    Ok(scheduler)
}

pub fn start_job_scheduler(app: &AppHandle) -> LocalResult<()> {
    recover_pending_job_deletions(app)?;
    job_scheduler(app).map(|_| ())
}

pub fn shutdown_job_scheduler() -> LocalResult<()> {
    match JOB_SCHEDULER.get() {
        Some(scheduler) => scheduler.shutdown(Duration::from_secs(5)),
        None => Ok(()),
    }
}

fn safe_job_dir(app: &AppHandle, job_id: &str) -> LocalResult<PathBuf> {
    runner_files::resolve_job_dir(&local_db::jobs_root(app)?, job_id)
}

fn recover_pending_job_deletions(app: &AppHandle) -> LocalResult<()> {
    let operations = local_db::list_pending_job_deletions(app)?;
    let mut failures = Vec::new();
    for operation in operations {
        if let Err(error) = converge_job_deletion(app, &operation) {
            let _ = local_db::record_job_deletion_error(app, &operation.operation_id, &error);
            failures.push(format!("{}: {error}", operation.job_id));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("恢复未完成的任务删除失败: {}", failures.join("; ")))
    }
}

fn converge_job_deletion(
    app: &AppHandle,
    operation: &local_db::JobDeletionOperation,
) -> LocalResult<()> {
    runner_process::terminate_persisted_runner(
        operation.runner_pid,
        operation.runner_process_identity.as_deref(),
    )?;
    if local_db::job_exists(app, &operation.job_id)? {
        local_db::fence_local_job_run(app, &operation.job_id)?;
    }
    local_db::mark_job_deletion_phase(
        app,
        &operation.operation_id,
        local_db::JobDeletionPhase::Fenced,
    )?;

    let jobs_root = local_db::jobs_root(app)?;
    runner_files::move_job_dir_to_trash(&jobs_root, &operation.job_id, &operation.trash_name)?;
    local_db::mark_job_deletion_phase(
        app,
        &operation.operation_id,
        local_db::JobDeletionPhase::Trashed,
    )?;
    local_db::delete_job_for_operation(app, operation)?;
    runner_files::purge_job_trash(&jobs_root, &operation.trash_name)?;
    local_db::complete_job_deletion_operation(app, &operation.operation_id)
}

fn derived_duration_minutes(
    runner_duration_minutes: Option<u32>,
    fallback_duration_minutes: u32,
    transcript_segments: &[TranscriptSegment],
    speaker_segments: &[TranscriptSegment],
) -> u32 {
    runner_duration_minutes
        .filter(|value| *value > 0)
        .or_else(|| derive_duration_minutes_from_segments(transcript_segments, speaker_segments))
        .unwrap_or(fallback_duration_minutes)
}

fn derive_duration_minutes_from_segments(
    transcript_segments: &[TranscriptSegment],
    speaker_segments: &[TranscriptSegment],
) -> Option<u32> {
    let max_end_ms = transcript_segments
        .iter()
        .chain(speaker_segments.iter())
        .map(|segment| segment.end_ms)
        .max()?;

    if max_end_ms == 0 {
        return None;
    }

    Some(((max_end_ms as f64) / 60_000.0).ceil() as u32)
}

fn read_runner_result(job_dir: &Path) -> LocalResult<RunnerResult> {
    runner_files::read_json_file(job_dir, "result.json")
}

fn read_progress_snapshot(job_dir: &Path) -> Option<ProgressSnapshot> {
    runner_files::read_optional_json_file(job_dir, "progress.json")
}

fn read_runner_failure_reason(job_dir: &Path) -> Option<String> {
    let progress = read_progress_snapshot(job_dir)?;
    if progress.stage != "failed" {
        return None;
    }

    progress
        .failure_reason
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            progress
                .status_message
                .filter(|value| !value.trim().is_empty())
        })
}

fn empty_summary() -> MeetingSummary {
    MeetingSummary::default()
}

fn make_job_id() -> String {
    format!(
        "job-{}-{}",
        unix_timestamp_millis(),
        JOB_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
