//! Core domain model and policies.
//!
//! Domain modules own state transitions, value objects, and errors that should
//! not depend on Tauri, SQLite, HTTP, or operating-system APIs.
pub mod asr;
pub mod asr_resources;
pub mod dashboard;
pub mod error;
pub mod job;
pub mod meeting_minutes;
pub mod platform;
pub mod runtime;
pub mod settings;
pub mod transcript;
