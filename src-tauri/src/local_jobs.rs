use crate::local_db::{
    self, job_dir, mark_job_processing_started, update_job_completion, update_job_failure,
    update_job_process_log, update_job_statuses, AppSettings, MeetingJob, MeetingSourceFile,
    MeetingSummary, TranscriptSegment,
};
use serde::Deserialize;
use crate::local_runtime;
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

static JOB_COUNTER: AtomicU64 = AtomicU64::new(1);

type LocalResult<T> = Result<T, String>;
const MAX_LOCAL_ASR_THREADS: u32 = 32;

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
pub fn retry_job(
    app: AppHandle,
    id: String,
) -> LocalResult<MeetingJob> {
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
    let resolved_runtime =
        local_runtime::resolve_python_runtime(app, Some(&settings.python_path))?;
    let runner_script_path = resolve_runner_script_path(app, job.runner_script_path.as_deref())?;

    update_job_statuses(app, job_id, "transcribing", "idle", "transcribing", None)?;
    let processing_started_at_ms = unix_timestamp_millis() as u64;
    mark_job_processing_started(app, job_id, processing_started_at_ms)?;
    let runtime_threads = resolve_local_asr_threads(&settings);
    append_process_log_line(
        &dir,
        &format!(
            "[runner] source={}, device={}, threads={}, batch_size_s={}, speaker={}, ffmpeg={}",
            resolved_runtime.source_label,
            normalize_local_asr_device(&settings),
            runtime_threads,
            settings.local_asr_batch_size_seconds,
            if job.enable_speaker { "true" } else { "false" },
            resolved_runtime
                .ffmpeg_path
                .as_deref()
                .unwrap_or("not-configured")
        ),
    )?;

    validate_runtime_tools_for_job(&dir, &input_file, resolved_runtime.ffmpeg_path.as_deref())?;
    sync_process_log(app, job_id, &dir)?;

    let mut command = Command::new(&resolved_runtime.python_path);
    command
        .env("OMP_NUM_THREADS", runtime_threads.to_string())
        .env("MKL_NUM_THREADS", runtime_threads.to_string())
        .env("NUMEXPR_NUM_THREADS", runtime_threads.to_string())
        .env("KMP_DUPLICATE_LIB_OK", "TRUE")
        .env("FUNASR_DEVICE", normalize_local_asr_device(&settings))
        .env(
            "FUNASR_BATCH_SIZE_S",
            settings.local_asr_batch_size_seconds.to_string(),
        )
        .arg(&runner_script_path)
        .arg("--job-dir")
        .arg(&dir)
        .arg("--input")
        .arg(&input_file)
        .arg("--lang")
        .arg(&job.lang)
        .arg("--speaker")
        .arg(if job.enable_speaker { "true" } else { "false" })
        .arg("--hotwords")
        .arg(job.hotwords.join(","));

    if let Some(ffmpeg_path) = resolved_runtime.ffmpeg_path.as_deref() {
        command.env("LIBERTY_FFMPEG_PATH", ffmpeg_path);
    }

    if let Some(models_root) = resolved_runtime.models_root.as_deref() {
        command
            .env("MODELSCOPE_CACHE", Path::new(models_root).join("modelscope"))
            .env("HF_HOME", Path::new(models_root).join("huggingface"))
            .env("TORCH_HOME", Path::new(models_root).join("torch"));
    }

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("无法启动本地 Python 处理进程: {err}"))?;
    let status = stream_child_logs(app, job_id, &dir, &mut child)?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        let detailed = read_runner_failure_reason(&dir)
            .or_else(|| summarize_process_log(&dir, 12))
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
    let processing_duration_seconds = Some(
        ((processing_finished_at_ms.saturating_sub(processing_started_at_ms)) / 1000) as u32,
    );
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

fn validate_runtime_tools_for_job(
    job_dir: &Path,
    input_file: &str,
    ffmpeg_path: Option<&str>,
) -> LocalResult<()> {
    let input_suffix = Path::new(input_file)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    if input_suffix == "wav" {
        return Ok(());
    }

    let ffmpeg = ffmpeg_path
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "当前文件需要 ffmpeg 进行音频解码，但本地运行环境中未找到 ffmpeg。".to_string())?;

    append_process_log_line(job_dir, &format!("[runner] validating ffmpeg={ffmpeg}"))?;
    let output = Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-version")
        .output()
        .map_err(|err| {
            format!(
                "任务启动前检测 ffmpeg 失败：{}。请重新安装本地运行环境。",
                err
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    if !output.stdout.is_empty() {
        append_process_log_line(job_dir, &String::from_utf8_lossy(&output.stdout))?;
    }
    if !output.stderr.is_empty() {
        append_process_log_line(job_dir, &String::from_utf8_lossy(&output.stderr))?;
    }

    Err(format!(
        "任务启动前检测 ffmpeg 失败，退出码 {}。请重新安装本地运行环境。",
        output.status.code().unwrap_or(-1)
    ))
}

fn stream_child_logs(
    app: &AppHandle,
    job_id: &str,
    job_dir: &Path,
    child: &mut std::process::Child,
) -> LocalResult<ExitStatus> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "本地 Python 处理进程未返回 stdout 管道。".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "本地 Python 处理进程未返回 stderr 管道。".to_string())?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    spawn_log_reader(stdout, tx.clone());
    spawn_log_reader(stderr, tx.clone());
    drop(tx);

    let mut last_sync_at = Instant::now();
    while let Ok(chunk) = rx.recv() {
        append_process_log(job_dir, &chunk)?;
        if last_sync_at.elapsed() >= Duration::from_millis(400) {
            sync_process_log(app, job_id, job_dir)?;
            last_sync_at = Instant::now();
        }
    }

    let status = child.wait().map_err(|err| err.to_string())?;
    sync_process_log(app, job_id, job_dir)?;
    Ok(status)
}

fn spawn_log_reader<R>(mut stream: R, tx: mpsc::Sender<Vec<u8>>)
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if tx.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
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

    if let Some(path) = configured_path.map(str::trim).filter(|value| !value.is_empty()) {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join("funasr_runner.py"));
        candidates.push(resource_dir.join("funasr_runner.py"));
        candidates.push(resource_dir.join("_up_").join("scripts").join("funasr_runner.py"));
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("../scripts/funasr_runner.py"));
    candidates.push(manifest_dir.join("scripts/funasr_runner.py"));

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("scripts/funasr_runner.py"));
        candidates.push(current_dir.join("../scripts/funasr_runner.py"));
    }

    if let Ok(executable_path) = std::env::current_exe() {
        if let Some(executable_dir) = executable_path.parent() {
            candidates.push(executable_dir.join("scripts/funasr_runner.py"));
            candidates.push(executable_dir.join("../Resources/scripts/funasr_runner.py"));
            candidates.push(executable_dir.join("../Resources/funasr_runner.py"));
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            let resolved = candidate.canonicalize().unwrap_or(candidate);
            return Ok(resolved.to_string_lossy().into_owned());
        }
    }

    Err("未找到内置 Runner 脚本，请检查应用资源或项目 scripts 目录。".into())
}

fn normalize_local_asr_device(settings: &AppSettings) -> String {
    match settings.local_asr_device.as_str() {
        "cpu" => "cpu".into(),
        "mps" => "mps".into(),
        "cuda" => "cuda".into(),
        _ => "auto".into(),
    }
}

fn resolve_local_asr_threads(settings: &AppSettings) -> u32 {
    if settings.local_asr_threads > 0 {
        return settings.local_asr_threads.clamp(1, MAX_LOCAL_ASR_THREADS);
    }

    let available = std::thread::available_parallelism()
        .map(|value| value.get() as u32)
        .unwrap_or(4);

    available.clamp(1, MAX_LOCAL_ASR_THREADS)
}

fn append_process_log(job_dir: &Path, bytes: &[u8]) -> LocalResult<()> {
    if bytes.is_empty() {
        return Ok(());
    }

    let log_path = job_dir.join("process.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|err| err.to_string())?;
    file.write_all(bytes).map_err(|err| err.to_string())
}

fn append_process_log_line(job_dir: &Path, line: &str) -> LocalResult<()> {
    append_process_log(job_dir, format!("{line}\n").as_bytes())
}

fn reset_process_log(job_dir: &Path) -> LocalResult<()> {
    fs::write(job_dir.join("process.log"), []).map_err(|err| err.to_string())
}

fn reset_runner_files(job_dir: &Path) -> LocalResult<()> {
    for name in ["result.json", "progress.json", "job.json"] {
        let path = job_dir.join(name);
        if path.exists() {
            fs::remove_file(path).map_err(|err| err.to_string())?;
        }
    }

    Ok(())
}

fn sync_process_log(app: &AppHandle, job_id: &str, job_dir: &Path) -> LocalResult<()> {
    let log = fs::read_to_string(job_dir.join("process.log"))
        .unwrap_or_default()
        .trim()
        .to_string();
    update_job_process_log(app, job_id, &log)
}

fn mark_failed(app: &AppHandle, job_id: &str, reason: &str) -> LocalResult<()> {
    let dir = job_dir(app, job_id)?;
    sync_process_log(app, job_id, &dir)?;
    let detailed_reason = read_runner_failure_reason(&dir)
        .or_else(|| summarize_process_log(&dir, 12))
        .unwrap_or_else(|| reason.to_string());
    let job = local_db::get_job(app, job_id)?;
    let processing_finished_at_ms = unix_timestamp_millis() as u64;
    let processing_duration_seconds = job.processing_started_at_ms.map(|started_at| {
        ((processing_finished_at_ms.saturating_sub(started_at)) / 1000) as u32
    });
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
    let bytes = fs::read(job_dir.join("result.json")).map_err(|err| err.to_string())?;
    serde_json::from_slice(&bytes).map_err(|err| err.to_string())
}

fn read_progress_snapshot(job_dir: &Path) -> Option<ProgressSnapshot> {
    let bytes = fs::read(job_dir.join("progress.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn read_runner_failure_reason(job_dir: &Path) -> Option<String> {
    let progress = read_progress_snapshot(job_dir)?;
    if progress.stage != "failed" {
      return None;
    }

    progress
        .failure_reason
        .filter(|value| !value.trim().is_empty())
        .or_else(|| progress.status_message.filter(|value| !value.trim().is_empty()))
}

fn summarize_process_log(job_dir: &Path, max_lines: usize) -> Option<String> {
    let log = fs::read_to_string(job_dir.join("process.log")).ok()?;
    let lines: Vec<&str> = log.lines().filter(|line| !line.trim().is_empty()).collect();

    if lines.is_empty() {
        return None;
    }

    let tail = lines
        .iter()
        .rev()
        .take(max_lines)
        .copied()
        .collect::<Vec<_>>();
    Some(tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
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

fn unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
