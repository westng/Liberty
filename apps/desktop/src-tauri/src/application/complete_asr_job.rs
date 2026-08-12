use crate::{
    domain::asr::{validate_runner_result, NormalizedRunnerResult},
    local_db::TranscriptSegment,
};

pub struct CompleteAsrJobRequest {
    pub job_id: String,
    pub attempt_id: u64,
    pub lease_token: u64,
    pub result: NormalizedRunnerResult,
    pub fallback_duration_minutes: u32,
    pub processing_finished_at_ms: u64,
    pub processing_duration_seconds: Option<u32>,
    pub process_log: String,
}

pub struct AsrJobCompletion {
    pub job_id: String,
    pub attempt_id: u64,
    pub lease_token: u64,
    pub result: NormalizedRunnerResult,
    pub duration_minutes: u32,
    pub processing_finished_at_ms: u64,
    pub processing_duration_seconds: Option<u32>,
    pub process_log: String,
}

pub trait AsrJobCompletionPort {
    fn complete(&self, completion: &AsrJobCompletion) -> Result<bool, String>;
}

pub fn complete_asr_job(
    port: &dyn AsrJobCompletionPort,
    request: CompleteAsrJobRequest,
) -> Result<bool, String> {
    validate_runner_result(&request.result)
        .map_err(|error| format!("runner_result_invalid: {error}"))?;
    let duration_minutes = derive_duration_minutes(
        request.result.duration_minutes,
        request.fallback_duration_minutes,
        &request.result.transcript_segments,
        &request.result.speaker_segments,
    );
    port.complete(&AsrJobCompletion {
        job_id: request.job_id,
        attempt_id: request.attempt_id,
        lease_token: request.lease_token,
        result: request.result,
        duration_minutes,
        processing_finished_at_ms: request.processing_finished_at_ms,
        processing_duration_seconds: request.processing_duration_seconds,
        process_log: request.process_log,
    })
}

fn derive_duration_minutes(
    runner_duration_minutes: Option<u32>,
    fallback_duration_minutes: u32,
    transcript_segments: &[TranscriptSegment],
    speaker_segments: &[TranscriptSegment],
) -> u32 {
    runner_duration_minutes
        .filter(|value| *value > 0)
        .or_else(|| {
            transcript_segments
                .iter()
                .chain(speaker_segments.iter())
                .map(|segment| segment.end_ms)
                .max()
                .filter(|end_ms| *end_ms > 0)
                .map(|end_ms| ((end_ms as f64) / 60_000.0).ceil() as u32)
        })
        .unwrap_or(fallback_duration_minutes)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::domain::asr::{AsrBackend, DiarizationStatus, RunnerWarning};

    use super::*;

    #[derive(Default)]
    struct RecordingPort {
        duration_minutes: RefCell<Option<u32>>,
    }

    impl AsrJobCompletionPort for RecordingPort {
        fn complete(&self, completion: &AsrJobCompletion) -> Result<bool, String> {
            self.duration_minutes
                .replace(Some(completion.duration_minutes));
            Ok(true)
        }
    }

    fn segment(end_ms: u64, speaker: Option<&str>) -> TranscriptSegment {
        TranscriptSegment {
            id: "segment-1".into(),
            start_ms: 0,
            end_ms,
            speaker: speaker.map(str::to_owned),
            text: "有效逐字稿".into(),
        }
    }

    #[test]
    fn validates_and_derives_duration_before_completing() {
        let port = RecordingPort::default();
        let accepted = complete_asr_job(
            &port,
            CompleteAsrJobRequest {
                job_id: "job-1".into(),
                attempt_id: 1,
                lease_token: 2,
                result: NormalizedRunnerResult {
                    protocol_version: 2,
                    asr_backend: AsrBackend::Funasr,
                    diarization_status: DiarizationStatus::Unavailable,
                    warnings: vec![RunnerWarning {
                        code: "diarization_unavailable".into(),
                        message: "当前后端不可用".into(),
                    }],
                    duration_minutes: None,
                    transcript_segments: vec![segment(60_001, None)],
                    speaker_segments: Vec::new(),
                },
                fallback_duration_minutes: 0,
                processing_finished_at_ms: 10,
                processing_duration_seconds: Some(1),
                process_log: String::new(),
            },
        )
        .expect("completion");

        assert!(accepted);
        assert_eq!(*port.duration_minutes.borrow(), Some(2));
    }

    #[test]
    fn rejects_contradictory_result_before_persistence() {
        let port = RecordingPort::default();
        let error = complete_asr_job(
            &port,
            CompleteAsrJobRequest {
                job_id: "job-1".into(),
                attempt_id: 1,
                lease_token: 2,
                result: NormalizedRunnerResult {
                    protocol_version: 2,
                    asr_backend: AsrBackend::Funasr,
                    diarization_status: DiarizationStatus::Completed,
                    warnings: Vec::new(),
                    duration_minutes: Some(1),
                    transcript_segments: vec![segment(1, None)],
                    speaker_segments: Vec::new(),
                },
                fallback_duration_minutes: 1,
                processing_finished_at_ms: 10,
                processing_duration_seconds: Some(1),
                process_log: String::new(),
            },
        )
        .expect_err("invalid completion");

        assert!(error.starts_with("runner_result_invalid:"));
        assert_eq!(*port.duration_minutes.borrow(), None);
    }
}
