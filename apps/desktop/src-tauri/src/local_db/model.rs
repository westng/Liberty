use serde::{Deserialize, Serialize};

use crate::infrastructure::time::unix_timestamp_millis;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<String>,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSummary {
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub action_items: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub follow_ups: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSourceFile {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    pub size_label: String,
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryActionItem {
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub due_date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryResult {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub action_items: Vec<AiSummaryActionItem>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub follow_ups: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiModelConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub api_key_ref: String,
    pub model: String,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub include_speaker_by_default: bool,
    pub include_timestamp_by_default: bool,
    pub builtin: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeetingMember {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub department: String,
    pub sort_order: i64,
    pub is_recorder: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetProfile {
    pub id: String,
    pub name: String,
    pub level: i64,
    pub experience: i64,
    pub stage: String,
    pub current_mood: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetSettings {
    pub pet_id: String,
    pub desktop_enabled: bool,
    pub always_on_top: bool,
    pub muted: bool,
    pub focus_mode_enabled: bool,
    pub proactive_level: i64,
    pub last_window_x: Option<f64>,
    pub last_window_y: Option<f64>,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetCosmeticUnlock {
    pub id: String,
    pub pet_id: String,
    pub cosmetic_type: String,
    pub cosmetic_key: String,
    pub unlocked_at: String,
    pub equipped: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetEventLedgerEntry {
    pub id: String,
    pub pet_id: String,
    pub event_type: String,
    pub event_source: String,
    pub event_value: i64,
    pub event_time: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetWallet {
    pub pet_id: String,
    pub currency_key: String,
    pub balance: i64,
    pub lifetime_earned: i64,
    pub lifetime_spent: i64,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetInventoryItem {
    pub id: String,
    pub pet_id: String,
    pub item_key: String,
    pub item_type: String,
    pub slot: String,
    pub quantity: i64,
    pub equipped: bool,
    pub source: String,
    pub purchased_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetEconomyEntry {
    pub id: String,
    pub pet_id: String,
    pub entry_type: String,
    pub currency_key: String,
    pub amount: i64,
    pub balance_after: i64,
    pub source_type: String,
    pub source_key: String,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetMilestoneCounter {
    pub pet_id: String,
    pub counter_key: String,
    pub counter_value: i64,
    pub last_event_key: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetStoreCatalogItem {
    pub item_key: String,
    pub item_type: String,
    pub slot: String,
    pub name_zh: String,
    pub name_en: String,
    pub description_zh: String,
    pub description_en: String,
    pub rarity: String,
    pub price_lp: i64,
    pub level_gate: i64,
    pub stage_gate: String,
    pub milestone_gate: String,
    pub asset_key: String,
    pub enabled: bool,
    pub sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetStoreCatalogItemState {
    pub item: PetStoreCatalogItem,
    pub owned: bool,
    pub equipped: bool,
    pub quantity: i64,
    pub growth_value: i64,
    pub purchasable: bool,
    pub locked_reason_zh: String,
    pub locked_reason_en: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetEquipmentState {
    pub current_pet: Option<PetInventoryItem>,
    pub accessory: Option<PetInventoryItem>,
    pub scene: Option<PetInventoryItem>,
    pub badge: Option<PetInventoryItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetStoreState {
    pub profile: PetProfile,
    pub wallet: PetWallet,
    pub catalog: Vec<PetStoreCatalogItemState>,
    pub inventory: Vec<PetInventoryItem>,
    pub equipment: PetEquipmentState,
    pub counters: Vec<PetMilestoneCounter>,
    pub economy: Vec<PetEconomyEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeetingMemberImportResult {
    pub created: usize,
    pub updated: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryRun {
    pub id: String,
    pub job_id: String,
    #[serde(default)]
    pub model_config_id: String,
    #[serde(default)]
    pub template_id: String,
    pub include_speaker: bool,
    pub include_timestamp: bool,
    #[serde(default)]
    pub extra_instructions: String,
    pub status: String,
    pub error_message: Option<String>,
    pub prompt_preview: Option<String>,
    pub raw_response: Option<String>,
    pub result: Option<AiSummaryResult>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeetingJob {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub source_files: Vec<MeetingSourceFile>,
    pub duration_minutes: u32,
    pub processing_started_at_ms: Option<u64>,
    pub processing_finished_at_ms: Option<u64>,
    pub processing_duration_seconds: Option<u32>,
    pub progress_percent: Option<u32>,
    pub progress_message: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub hotwords: Vec<String>,
    pub lang: String,
    pub enable_speaker: bool,
    pub summary_template: String,
    pub upload_status: String,
    pub asr_status: String,
    pub summary_status: String,
    pub overall_status: String,
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub transcript_segments: Vec<TranscriptSegment>,
    #[serde(default)]
    pub speaker_segments: Vec<TranscriptSegment>,
    #[serde(default)]
    pub summary: MeetingSummary,
    #[serde(default)]
    pub summary_runs: Vec<AiSummaryRun>,
    pub active_summary_run_id: Option<String>,
    #[serde(default)]
    pub export_formats: Vec<String>,
    pub last_exported_at: Option<String>,
    pub process_log: Option<String>,
    pub python_path: Option<String>,
    pub runner_script_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme_mode: String,
    pub liquid_glass_style: String,
    pub accent_color: String,
    pub locale: String,
    pub backend_url: String,
    pub api_token: String,
    pub default_hotwords: String,
    pub summary_template: String,
    pub concurrency: u32,
    pub python_path: String,
    pub runner_script_path: String,
    pub local_asr_device: String,
    pub local_asr_threads: u32,
    pub local_asr_batch_size_seconds: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManagedRuntimeState {
    pub platform_id: String,
    pub runtime_version: String,
    pub python_version: String,
    pub status: String,
    pub python_executable_path: Option<String>,
    pub models_root: Option<String>,
    pub install_root: Option<String>,
    pub last_error: Option<String>,
    pub installed_at: Option<String>,
    pub updated_at: String,
    pub last_log_path: Option<String>,
}

impl ManagedRuntimeState {
    pub fn missing(platform_id: &str, runtime_version: &str, python_version: &str) -> Self {
        Self {
            platform_id: platform_id.to_string(),
            runtime_version: runtime_version.to_string(),
            python_version: python_version.to_string(),
            status: "missing".into(),
            python_executable_path: None,
            models_root: None,
            install_root: None,
            last_error: None,
            installed_at: None,
            updated_at: unix_timestamp_millis().to_string(),
            last_log_path: None,
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: "auto".into(),
            liquid_glass_style: "transparent".into(),
            accent_color: "#2f6dff".into(),
            locale: "zh-CN".into(),
            backend_url: String::new(),
            api_token: String::new(),
            default_hotwords: "SeACo-Paraformer, FunASR, 会议纪要".into(),
            summary_template: "表格版会议纪要".into(),
            concurrency: 2,
            python_path: String::new(),
            runner_script_path: String::new(),
            local_asr_device: "auto".into(),
            local_asr_threads: 0,
            local_asr_batch_size_seconds: 300,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProgressSnapshot {
    pub(crate) stage: String,
    pub(crate) status_message: Option<String>,
    pub(crate) failure_reason: Option<String>,
    pub(crate) progress_percent: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LegacyRunnerResult {
    pub(crate) duration_minutes: Option<u32>,
    pub(crate) transcript_segments: Option<Vec<TranscriptSegment>>,
    pub(crate) speaker_segments: Option<Vec<TranscriptSegment>>,
    pub(crate) failure_reason: Option<String>,
}
