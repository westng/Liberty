use serde::{Deserialize, Serialize};

use crate::domain::transcript::TranscriptSegment;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AsrBackend {
    Funasr,
    SherpaOnnx,
    #[default]
    Unknown,
}

impl AsrBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Funasr => "funasr",
            Self::SherpaOnnx => "sherpa-onnx",
            Self::Unknown => "unknown",
        }
    }
}

impl TryFrom<&str> for AsrBackend {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "funasr" => Ok(Self::Funasr),
            "sherpa-onnx" => Ok(Self::SherpaOnnx),
            "unknown" => Ok(Self::Unknown),
            value => Err(format!("不支持的 ASR 后端: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiarizationStatus {
    Disabled,
    Pending,
    Processing,
    Completed,
    Unavailable,
    Failed,
    #[default]
    LegacyUnverified,
}

impl DiarizationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
            Self::LegacyUnverified => "legacy_unverified",
        }
    }

    pub fn is_verified(self) -> bool {
        self == Self::Completed
    }
}

impl TryFrom<&str> for DiarizationStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "disabled" => Ok(Self::Disabled),
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "unavailable" => Ok(Self::Unavailable),
            "failed" => Ok(Self::Failed),
            "legacy_unverified" => Ok(Self::LegacyUnverified),
            value => Err(format!("不支持的说话人状态: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnerWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedRunnerResult {
    pub protocol_version: u32,
    pub asr_backend: AsrBackend,
    pub diarization_status: DiarizationStatus,
    pub warnings: Vec<RunnerWarning>,
    pub duration_minutes: Option<u32>,
    pub transcript_segments: Vec<TranscriptSegment>,
    pub speaker_segments: Vec<TranscriptSegment>,
}

pub fn validate_runner_result(result: &NormalizedRunnerResult) -> Result<(), String> {
    if result.transcript_segments.is_empty() {
        return Err("Runner 未返回可用逐字稿。".into());
    }

    match result.diarization_status {
        DiarizationStatus::Completed => {
            if result.speaker_segments.is_empty() {
                return Err("Runner 声明说话人分离完成，但未返回说话人分段。".into());
            }
            if result.speaker_segments.iter().any(|segment| {
                segment
                    .speaker
                    .as_deref()
                    .is_none_or(|speaker| speaker.trim().is_empty() || speaker == "Speaker 1")
            }) {
                return Err("Runner 声明说话人分离完成，但包含空标签或默认 Speaker 1。".into());
            }
        }
        DiarizationStatus::Disabled
        | DiarizationStatus::Unavailable
        | DiarizationStatus::Failed => {
            if !result.speaker_segments.is_empty() {
                return Err("Runner 的说话人状态与分段内容矛盾。".into());
            }
        }
        DiarizationStatus::LegacyUnverified => {}
        DiarizationStatus::Pending | DiarizationStatus::Processing => {
            return Err("Runner 成功结果不能处于待处理的说话人状态。".into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(speaker: Option<&str>) -> TranscriptSegment {
        TranscriptSegment {
            id: "segment-1".into(),
            start_ms: 0,
            end_ms: 100,
            speaker: speaker.map(str::to_string),
            text: "逐字稿".into(),
        }
    }

    #[test]
    fn completed_requires_non_default_speaker_labels() {
        let result = NormalizedRunnerResult {
            protocol_version: 2,
            asr_backend: AsrBackend::Funasr,
            diarization_status: DiarizationStatus::Completed,
            warnings: Vec::new(),
            duration_minutes: Some(1),
            transcript_segments: vec![segment(None)],
            speaker_segments: vec![segment(Some("Speaker 1"))],
        };

        assert!(validate_runner_result(&result).is_err());
    }

    #[test]
    fn unavailable_preserves_transcript_without_speaker_projection() {
        let result = NormalizedRunnerResult {
            protocol_version: 2,
            asr_backend: AsrBackend::SherpaOnnx,
            diarization_status: DiarizationStatus::Unavailable,
            warnings: vec![RunnerWarning {
                code: "diarization_unavailable".into(),
                message: "当前后端不支持。".into(),
            }],
            duration_minutes: Some(1),
            transcript_segments: vec![segment(None)],
            speaker_segments: Vec::new(),
        };

        assert!(validate_runner_result(&result).is_ok());
    }
}
