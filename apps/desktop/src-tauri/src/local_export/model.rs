#[derive(Debug, Clone, Default)]
pub struct ExportDocData {
    pub title: String,
    pub meeting_name: String,
    pub meeting_time: String,
    pub meeting_location: String,
    pub recorder: String,
    pub attendees: String,
    pub absentees: String,
    pub topics: String,
    pub host: String,
    pub reviewer: String,
    pub closing_summary: Vec<String>,
    pub fallback_overview: Vec<String>,
    pub speech_blocks: Vec<SpeechBlock>,
}

#[derive(Debug, Clone, Default)]
pub struct SpeechBlock {
    pub department: String,
    pub name: String,
    pub weekly_summary: Vec<String>,
    pub next_week_plan: Vec<String>,
    pub summary: Vec<String>,
    pub original_index: usize,
}
