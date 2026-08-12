//! Application use cases.
//!
//! Use cases coordinate domain policy with infrastructure implementations. They
//! should be callable from Tauri commands and eventually from tests without a
//! webview or window context.
pub mod complete_asr_job;
mod credential_cleanup;
pub mod delete_ai_model;
pub mod project_meeting_minutes;
pub mod save_ai_model;
pub mod save_settings;
pub mod switch_runtime_source;
