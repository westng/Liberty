use crate::infrastructure::process_logs;
use crate::infrastructure::runner_files;
use crate::infrastructure::runner_process::{self, RunnerCommandInput};
use crate::infrastructure::time::unix_timestamp_millis;
use crate::local_db::{
    self, job_dir, mark_job_processing_started, update_job_completion, update_job_failure,
    update_job_process_log, update_job_statuses, MeetingJob, MeetingSourceFile, MeetingSummary,
    TranscriptSegment,
};
use crate::local_runtime;
use crate::process_utils::configure_background_process;
use serde::Deserialize;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
};
use tauri::{AppHandle, Manager};

static JOB_COUNTER: AtomicU64 = AtomicU64::new(1);

type LocalResult<T> = Result<T, String>;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobInput {
    pub title: String,
    pub files: Vec<MeetingSourceFile>,
    pub hotwords: Vec<String>,
    pub lang: String,
    pub enable_speaker: bool,
    pub summary_template: String,
    pub created_at: String,
    #[serde(default)]
    pub runner_script_path: String,
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
    local_db::get_job(&app, &id)
}

#[tauri::command]
pub fn get_job_result(app: AppHandle, id: String) -> LocalResult<MeetingJob> {
    local_db::get_job(&app, &id)
}

#[tauri::command]
pub fn rename_job_speaker(
    app: AppHandle,
    id: String,
    from_speaker: String,
    to_speaker: String,
) -> LocalResult<MeetingJob> {
    local_db::rename_job_speaker(&app, &id, &from_speaker, &to_speaker)?;
    local_db::get_job(&app, &id)
}

#[tauri::command]
pub fn delete_job(app: AppHandle, id: String) -> LocalResult<()> {
    let dir = job_dir(&app, &id)?;

    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| err.to_string())?;
    }

    local_db::delete_job(&app, &id)
}

#[tauri::command]
pub fn create_job(app: AppHandle, input: CreateJobInput) -> LocalResult<MeetingJob> {
    validate_create_input(&app, &input)?;

    let runner_script_path = resolve_runner_script_path(&app, Some(&input.runner_script_path))?;
    let job = build_initial_job(input, runner_script_path);
    let dir = job_dir(&app, &job.id)?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    reset_process_log(&dir)?;
    reset_runner_files(&dir)?;
    local_db::save_job_snapshot(&app, &job)?;
    spawn_local_job(app.clone(), job.id.clone());
    local_db::get_job(&app, &job.id)
}

#[tauri::command]
pub fn retry_job(app: AppHandle, id: String) -> LocalResult<MeetingJob> {
    let settings = local_db::get_settings(&app)?;
    let job = local_db::get_job(&app, &id)?;
    let first_file = job
        .source_files
        .first()
        .ok_or_else(|| "任务缺少输入文件。".to_string())?;

    if first_file.path.as_deref().unwrap_or("").trim().is_empty() {
        return Err("本地模式只支持带本地路径的文件。".into());
    }

    let dir = job_dir(&app, &id)?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    reset_process_log(&dir)?;
    reset_runner_files(&dir)?;
    let runner_script_path = resolve_runner_script_path(&app, job.runner_script_path.as_deref())?;
    let resolved_runtime =
        local_runtime::resolve_python_runtime(&app, Some(&settings.python_path))?;
    local_db::reset_job_for_retry(
        &app,
        &id,
        &resolved_runtime.python_path,
        &runner_script_path,
    )?;
    spawn_local_job(app.clone(), id.clone());
    local_db::get_job(&app, &id)
}

fn spawn_local_job(app: AppHandle, job_id: String) {
    std::thread::spawn(move || {
        if let Err(error) = execute_local_job(&app, &job_id) {
            let _ = mark_failed(&app, &job_id, &error);
        }
    });
}

fn execute_local_job(app: &AppHandle, job_id: &str) -> LocalResult<()> {
    let dir = job_dir(app, job_id)?;
    let job = local_db::get_job(app, job_id)?;
    let settings = local_db::get_settings(app)?;
    let input_file = job
        .source_files
        .first()
        .and_then(|file| file.path.clone())
        .ok_or_else(|| "任务缺少可处理的本地文件路径。".to_string())?;
    let resolved_runtime = local_runtime::resolve_python_runtime(app, Some(&settings.python_path))?;
    let runner_script_path = resolve_runner_script_path(app, job.runner_script_path.as_deref())?;

    update_job_statuses(app, job_id, "transcribing", "idle", "transcribing", None)?;
    let processing_started_at_ms = unix_timestamp_millis() as u64;
    mark_job_processing_started(app, job_id, processing_started_at_ms)?;
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
    sync_process_log(app, job_id, &dir)?;

    let mut command = runner_process::build_runner_command(RunnerCommandInput {
        runtime: &resolved_runtime,
        settings: &settings,
        job: &job,
        job_dir: &dir,
        runner_script_path: Path::new(&runner_script_path),
        input_file: Path::new(&input_file),
        runtime_threads,
    });

    configure_background_process(&mut command);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("无法启动本地 Python 处理进程: {err}"))?;
    let status =
        runner_process::stream_child_logs(app, job_id, &dir, &mut child, sync_process_log)?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        let detailed = read_runner_failure_reason(&dir)
            .or_else(|| process_logs::summarize_tail(&dir, 12))
            .unwrap_or_else(|| format!("本地 Python 处理失败，退出码 {code}。"));
        return Err(detailed);
    }

    let runner_result = read_runner_result(&dir)?;
    if let Some(reason) = runner_result.failure_reason.clone() {
        return Err(reason);
    }

    let transcript_segments = runner_result.transcript_segments.unwrap_or_default();
    let speaker_segments = runner_result
        .speaker_segments
        .unwrap_or_else(|| transcript_segments.clone());

    if job.enable_speaker && speaker_segments.is_empty() {
        return Err("Runner 未返回说话人分离结果。".into());
    }

    local_db::replace_job_segments(app, job_id, &transcript_segments, &speaker_segments)?;
    let processing_finished_at_ms = unix_timestamp_millis() as u64;
    let processing_duration_seconds =
        Some(((processing_finished_at_ms.saturating_sub(processing_started_at_ms)) / 1000) as u32);
    update_job_completion(
        app,
        job_id,
        derived_duration_minutes(
            runner_result.duration_minutes,
            job.duration_minutes,
            &transcript_segments,
            &speaker_segments,
        ),
        processing_finished_at_ms,
        processing_duration_seconds,
        None,
    )?;
    sync_process_log(app, job_id, &dir)?;

    Ok(())
}

fn build_initial_job(input: CreateJobInput, runner_script_path: String) -> MeetingJob {
    MeetingJob {
        id: make_job_id(),
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

fn validate_create_input(app: &AppHandle, input: &CreateJobInput) -> LocalResult<()> {
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

    let settings = local_db::get_settings(app)?;
    local_runtime::resolve_python_runtime(app, Some(&settings.python_path))?;

    Ok(())
}

fn resolve_runner_script_path(
    app: &AppHandle,
    configured_path: Option<&str>,
) -> LocalResult<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(path) = configured_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        candidates.push(PathBuf::from(path));
    }

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

fn reset_process_log(job_dir: &Path) -> LocalResult<()> {
    process_logs::reset(job_dir)
}

fn reset_runner_files(job_dir: &Path) -> LocalResult<()> {
    runner_files::reset_runner_files(job_dir)
}

fn sync_process_log(app: &AppHandle, job_id: &str, job_dir: &Path) -> LocalResult<()> {
    let log = process_logs::read_trimmed(job_dir);
    update_job_process_log(app, job_id, &log)
}

fn mark_failed(app: &AppHandle, job_id: &str, reason: &str) -> LocalResult<()> {
    let dir = job_dir(app, job_id)?;
    sync_process_log(app, job_id, &dir)?;
    let detailed_reason = read_runner_failure_reason(&dir)
        .or_else(|| process_logs::summarize_tail(&dir, 12))
        .unwrap_or_else(|| reason.to_string());
    let job = local_db::get_job(app, job_id)?;
    let processing_finished_at_ms = unix_timestamp_millis() as u64;
    let processing_duration_seconds = job
        .processing_started_at_ms
        .map(|started_at| ((processing_finished_at_ms.saturating_sub(started_at)) / 1000) as u32);
    update_job_failure(
        app,
        job_id,
        processing_finished_at_ms,
        processing_duration_seconds,
        &detailed_reason,
    )
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
