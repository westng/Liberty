pub mod redaction;
pub mod rotating_log;

use std::collections::BTreeMap;

use serde::Serialize;

use self::redaction::sanitize_diagnostic_text;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    pub level: DiagnosticLevel,
    pub code: String,
    pub message: String,
    pub context: BTreeMap<String, String>,
}

impl DiagnosticEvent {
    pub fn sanitized_line(&self, max_chars: usize) -> String {
        let serialized = serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"code":"diagnostic_serialization_failed"}"#.into());
        sanitize_diagnostic_text(&serialized, max_chars)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub code: String,
    pub actor: String,
    pub outcome: String,
    pub context: BTreeMap<String, String>,
}

impl AuditEvent {
    pub fn sanitized_line(&self, max_chars: usize) -> String {
        let serialized = serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"code":"audit_serialization_failed"}"#.into());
        sanitize_diagnostic_text(&serialized, max_chars)
    }
}

pub fn emit_audit(event: &AuditEvent) {
    eprintln!("[audit] {}", event.sanitized_line(4096));
}
