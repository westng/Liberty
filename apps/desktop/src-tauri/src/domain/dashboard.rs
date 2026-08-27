use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardOverview {
    pub range: String,
    pub trend_granularity: String,
    pub metrics: DashboardMetrics,
    pub trend: Vec<DashboardTrendPoint>,
    pub attention_jobs: Vec<DashboardJobSummary>,
    pub recent_results: Vec<DashboardJobSummary>,
    pub resources: DashboardResourceSummary,
    pub companion: Option<DashboardCompanionSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardMetrics {
    pub total_jobs: i64,
    pub media_duration_minutes: i64,
    pub processing_duration_seconds: i64,
    pub active_jobs: i64,
    pub completed_jobs: i64,
    pub failed_jobs: i64,
    pub transcript_ready_jobs: i64,
    pub speaker_eligible_jobs: i64,
    pub speaker_ready_jobs: i64,
    pub summary_ready_jobs: i64,
    pub exported_jobs: i64,
    pub warning_jobs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTrendPoint {
    pub period: String,
    pub total_jobs: i64,
    pub completed_jobs: i64,
    pub failed_jobs: i64,
    pub media_duration_minutes: i64,
    pub processing_duration_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardJobSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub duration_minutes: i64,
    pub overall_status: String,
    pub diarization_status: String,
    pub warning_count: usize,
    pub has_summary: bool,
    pub last_exported_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardResourceSummary {
    pub ai_models: i64,
    pub enabled_ai_models: i64,
    pub templates: i64,
    pub members: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardCompanionSummary {
    pub name: String,
    pub level: i64,
    pub current_level_experience: i64,
    pub next_level_experience: i64,
    pub level_progress_percent: i64,
    pub lp_balance: i64,
    pub checked_in_today: bool,
    pub claimable_activities: i64,
}
