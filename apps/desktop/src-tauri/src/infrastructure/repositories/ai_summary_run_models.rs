#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSummaryRunLease {
    pub run_id: String,
    pub attempt_id: u64,
    pub lease_token: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSummaryChunkSeed {
    pub index: usize,
    pub sha256: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSummaryPendingChunk {
    pub index: usize,
    pub sha256: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSummaryExecutionRecord {
    pub run_id: String,
    pub job_id: String,
    pub model_config_id: String,
    pub transcript_revision: String,
    pub transcript_sha256: String,
    pub execution_snapshot_json: String,
    pub chunk_count: usize,
}

#[derive(Debug, Clone)]
pub struct NewAiSummaryExecution<'a> {
    pub transcript_revision: &'a str,
    pub transcript_sha256: &'a str,
    pub transcript_snapshot_json: &'a str,
    pub execution_snapshot_json: &'a str,
    pub chunks: &'a [AiSummaryChunkSeed],
}

#[derive(Debug, Clone)]
pub struct AiSummaryCompletion<'a> {
    pub raw_response: &'a str,
    pub result_json: &'a str,
    pub minutes_payload_json: &'a str,
    pub diagnostics_json: &'a str,
    pub completed_at: &'a str,
}
