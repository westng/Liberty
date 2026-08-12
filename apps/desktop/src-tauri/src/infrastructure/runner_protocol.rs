use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::Deserialize;
use serde_json::Value;
use typify::import_types;

use crate::{
    domain::asr::{
        validate_runner_result, AsrBackend, DiarizationStatus, NormalizedRunnerResult,
        RunnerWarning,
    },
    local_db::{LocalResult, TranscriptSegment},
};

mod result_v2 {
    use super::*;

    import_types!(schema = "../../../packages/shared-types/schemas/runner/v2/result.schema.json");
}

mod progress_v2 {
    use super::*;

    import_types!(schema = "../../../packages/shared-types/schemas/runner/v2/progress.schema.json");
}

mod event_v2 {
    use super::*;

    import_types!(schema = "../../../packages/shared-types/schemas/runner/v2/event.schema.json");
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunnerV1Result {
    duration_minutes: Option<u32>,
    transcript_segments: Option<Vec<TranscriptSegment>>,
    speaker_segments: Option<Vec<TranscriptSegment>>,
    failure_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NormalizedProgress {
    pub protocol_version: u32,
    pub revision: Option<u64>,
    pub stage: String,
    pub status_message: Option<String>,
    pub failure_reason: Option<String>,
    pub progress_percent: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct NormalizedEvent {
    pub event_type: String,
    pub level: String,
    pub code: String,
    pub message: String,
    pub revision: Option<u64>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunnerV1Progress {
    stage: String,
    status_message: Option<String>,
    failure_reason: Option<String>,
    progress_percent: Option<u32>,
}

pub fn read_result(
    path: &Path,
    diarization_requested: bool,
    legacy_backend: AsrBackend,
) -> LocalResult<NormalizedRunnerResult> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    parse_result(&bytes, diarization_requested, legacy_backend)
}

pub fn parse_result(
    bytes: &[u8],
    diarization_requested: bool,
    legacy_backend: AsrBackend,
) -> LocalResult<NormalizedRunnerResult> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("Runner 结果不是合法 JSON: {error}"))?;
    let protocol_version = value.get("protocolVersion").and_then(Value::as_u64);

    let result = match protocol_version {
        None => normalize_v1(value, diarization_requested, legacy_backend)?,
        Some(2) => normalize_v2(value)?,
        Some(version) => return Err(format!("不支持的 Runner 协议版本: {version}")),
    };
    validate_runner_result(&result)?;
    Ok(result)
}

pub fn read_progress(path: &Path) -> LocalResult<NormalizedProgress> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let progress = parse_progress(&bytes)?;
    accept_progress_revision(path, &progress)?;
    Ok(progress)
}

fn accept_progress_revision(path: &Path, progress: &NormalizedProgress) -> LocalResult<()> {
    if progress.protocol_version != 2 {
        return Ok(());
    }
    let revision = progress
        .revision
        .ok_or_else(|| "runner_progress_revision_missing: V2 进度缺少 revision。".to_string())?;
    static SEEN_REVISIONS: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
    let mut seen = SEEN_REVISIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "runner_progress_revision_lock_failed: revision fence 不可用。".to_string())?;
    let key = path.to_path_buf();
    if seen.get(&key).is_some_and(|current| revision < *current) {
        return Err(format!(
            "runner_progress_stale: revision {revision} 小于已读取 revision {}。",
            seen[&key]
        ));
    }
    seen.insert(key, revision);
    Ok(())
}

pub fn parse_progress(bytes: &[u8]) -> LocalResult<NormalizedProgress> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("Runner 进度不是合法 JSON: {error}"))?;
    match value.get("protocolVersion").and_then(Value::as_u64) {
        None => {
            let progress: RunnerV1Progress = serde_json::from_value(value)
                .map_err(|error| format!("Runner V1 进度无效: {error}"))?;
            Ok(NormalizedProgress {
                protocol_version: 1,
                revision: None,
                stage: progress.stage,
                status_message: progress.status_message,
                failure_reason: progress.failure_reason,
                progress_percent: progress.progress_percent,
            })
        }
        Some(2) => {
            let progress: progress_v2::RunnerV2Progress = serde_json::from_value(value)
                .map_err(|error| protocol_error("progress_invalid", error))?;
            let progress = serde_json::to_value(progress)
                .map_err(|error| protocol_error("progress_normalization_failed", error))?;
            Ok(NormalizedProgress {
                protocol_version: 2,
                revision: Some(required_u64(&progress, "revision")?),
                stage: required_string(&progress, "stage")?,
                status_message: Some(required_string(&progress, "statusMessage")?),
                failure_reason: optional_string(&progress, "failureReason")?,
                progress_percent: optional_u32(&progress, "progressPercent")?,
            })
        }
        Some(version) => Err(format!("不支持的 Runner 进度协议版本: {version}")),
    }
}

pub fn parse_event(line: &str) -> LocalResult<NormalizedEvent> {
    let event: event_v2::RunnerV2Event =
        serde_json::from_str(line).map_err(|error| protocol_error("event_invalid", error))?;
    let event = serde_json::to_value(event)
        .map_err(|error| protocol_error("event_normalization_failed", error))?;
    Ok(NormalizedEvent {
        event_type: required_string(&event, "type")?,
        level: required_string(&event, "level")?,
        code: required_string(&event, "code")?,
        message: required_string(&event, "message")?,
        revision: optional_u64(&event, "revision")?,
        timestamp: optional_string(&event, "timestamp")?,
    })
}

fn normalize_v1(
    value: Value,
    diarization_requested: bool,
    legacy_backend: AsrBackend,
) -> LocalResult<NormalizedRunnerResult> {
    let result: RunnerV1Result =
        serde_json::from_value(value).map_err(|error| format!("Runner V1 结果无效: {error}"))?;
    if let Some(reason) = result
        .failure_reason
        .filter(|reason| !reason.trim().is_empty())
    {
        return Err(reason);
    }

    let transcript_segments = result.transcript_segments.unwrap_or_default();
    let speaker_segments = if diarization_requested {
        result.speaker_segments.unwrap_or_default()
    } else {
        Vec::new()
    };
    let (diarization_status, warnings) = if diarization_requested {
        (
            DiarizationStatus::LegacyUnverified,
            vec![RunnerWarning {
                code: "legacy_diarization_unverified".into(),
                message: "旧版 Runner 说话人结果无法验证，默认仅使用逐字稿。".into(),
            }],
        )
    } else {
        (DiarizationStatus::Disabled, Vec::new())
    };

    Ok(NormalizedRunnerResult {
        protocol_version: 1,
        asr_backend: legacy_backend,
        diarization_status,
        warnings,
        duration_minutes: result.duration_minutes,
        transcript_segments,
        speaker_segments,
    })
}

fn normalize_v2(value: Value) -> LocalResult<NormalizedRunnerResult> {
    let result: result_v2::RunnerV2Result =
        serde_json::from_value(value).map_err(|error| protocol_error("result_invalid", error))?;
    let result = serde_json::to_value(result)
        .map_err(|error| protocol_error("result_normalization_failed", error))?;
    let backend = AsrBackend::try_from(required_string(&result, "asrBackend")?.as_str())?;
    let status =
        DiarizationStatus::try_from(required_string(&result, "diarizationStatus")?.as_str())?;
    let warnings = required_array(&result, "warnings")?
        .iter()
        .map(|warning| {
            Ok(RunnerWarning {
                code: required_string(warning, "code")?,
                message: required_string(warning, "message")?,
            })
        })
        .collect::<LocalResult<Vec<_>>>()?;
    let transcript_segments = required_array(&result, "transcriptSegments")?
        .iter()
        .map(|segment| segment_from_v2(segment, false))
        .collect::<LocalResult<Vec<_>>>()?;
    let speaker_segments = required_array(&result, "speakerSegments")?
        .iter()
        .map(|segment| segment_from_v2(segment, true))
        .collect::<LocalResult<Vec<_>>>()?;

    Ok(NormalizedRunnerResult {
        protocol_version: 2,
        asr_backend: backend,
        diarization_status: status,
        warnings,
        duration_minutes: Some(required_u32(&result, "durationMinutes")?),
        transcript_segments,
        speaker_segments,
    })
}

fn segment_from_v2(value: &Value, include_speaker: bool) -> LocalResult<TranscriptSegment> {
    Ok(TranscriptSegment {
        id: required_string(value, "id")?,
        start_ms: required_u64(value, "startMs")?,
        end_ms: required_u64(value, "endMs")?,
        speaker: include_speaker
            .then(|| required_string(value, "speaker"))
            .transpose()?,
        text: required_string(value, "text")?,
    })
}

fn required_array<'a>(value: &'a Value, field: &str) -> LocalResult<&'a Vec<Value>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| protocol_error("normalization_invalid", format!("字段 {field} 不是数组")))
}

fn required_string(value: &Value, field: &str) -> LocalResult<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| protocol_error("normalization_invalid", format!("字段 {field} 不是字符串")))
}

fn optional_string(value: &Value, field: &str) -> LocalResult<Option<String>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_str().map(str::to_owned).map(Some).ok_or_else(|| {
            protocol_error(
                "normalization_invalid",
                format!("字段 {field} 不是字符串或 null"),
            )
        }),
    }
}

fn required_u64(value: &Value, field: &str) -> LocalResult<u64> {
    value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        protocol_error(
            "normalization_invalid",
            format!("字段 {field} 不是非负整数"),
        )
    })
}

fn required_u32(value: &Value, field: &str) -> LocalResult<u32> {
    u32::try_from(required_u64(value, field)?).map_err(|_| {
        protocol_error(
            "normalization_invalid",
            format!("字段 {field} 超出 u32 范围"),
        )
    })
}

fn optional_u64(value: &Value, field: &str) -> LocalResult<Option<u64>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            protocol_error(
                "normalization_invalid",
                format!("字段 {field} 不是非负整数或 null"),
            )
        }),
    }
}

fn optional_u32(value: &Value, field: &str) -> LocalResult<Option<u32>> {
    optional_u64(value, field)?
        .map(|number| {
            u32::try_from(number).map_err(|_| {
                protocol_error(
                    "normalization_invalid",
                    format!("字段 {field} 超出 u32 范围"),
                )
            })
        })
        .transpose()
}

fn protocol_error(code: &str, detail: impl std::fmt::Display) -> String {
    format!("runner_protocol_{code}: {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_v1_requested_speakers_to_legacy_unverified() {
        let result = parse_result(
            br#"{
                "durationMinutes": 1,
                "transcriptSegments": [{"id":"1","startMs":0,"endMs":1,"text":"ok"}],
                "speakerSegments": [{"id":"1","startMs":0,"endMs":1,"speaker":"Speaker 1","text":"ok"}],
                "failureReason": null
            }"#,
            true,
            AsrBackend::Funasr,
        )
        .expect("V1 result");

        assert_eq!(result.protocol_version, 1);
        assert_eq!(
            result.diarization_status,
            DiarizationStatus::LegacyUnverified
        );
        assert_eq!(result.speaker_segments.len(), 1);
    }

    #[test]
    fn rejects_v2_completed_with_default_speaker() {
        let result = parse_result(
            br#"{
                "protocolVersion": 2,
                "asrBackend": "funasr",
                "diarizationRequested": true,
                "diarizationStatus": "completed",
                "warnings": [],
                "durationMinutes": 1,
                "transcriptSegments": [{"id":"1","startMs":0,"endMs":1,"text":"ok"}],
                "speakerSegments": [{"id":"1","startMs":0,"endMs":1,"speaker":"Speaker 1","text":"ok"}]
            }"#,
            true,
            AsrBackend::Funasr,
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_unknown_v2_fields() {
        let result = parse_result(
            br#"{
                "protocolVersion": 2,
                "asrBackend": "sherpa-onnx",
                "diarizationRequested": true,
                "diarizationStatus": "unavailable",
                "warnings": [],
                "durationMinutes": 1,
                "transcriptSegments": [{"id":"1","startMs":0,"endMs":1,"text":"ok"}],
                "speakerSegments": [],
                "unexpected": true
            }"#,
            true,
            AsrBackend::SherpaOnnx,
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_stale_v2_progress_revision_for_same_attempt_file() {
        let root = std::env::temp_dir().join(format!(
            "liberty-progress-revision-{}-{}",
            std::process::id(),
            crate::infrastructure::time::unix_timestamp_millis()
        ));
        std::fs::create_dir_all(&root).expect("progress directory");
        let path = root.join("progress.json");
        let progress = |revision| {
            format!(
                r#"{{"protocolVersion":2,"revision":{revision},"stage":"transcribing","statusMessage":"running","failureReason":null,"progressPercent":25,"updatedAt":"2026-08-12T00:00:00Z"}}"#
            )
        };
        std::fs::write(&path, progress(2)).expect("new progress");
        assert_eq!(read_progress(&path).expect("revision 2").revision, Some(2));

        std::fs::write(&path, progress(1)).expect("stale progress");
        let error = read_progress(&path).expect_err("stale revision must fail");
        assert!(error.contains("runner_progress_stale"));
        let _ = std::fs::remove_dir_all(root);
    }
}
