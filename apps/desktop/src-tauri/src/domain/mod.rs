//! Core domain model and policies.
//!
//! Domain modules own state transitions, value objects, and errors that should
//! not depend on Tauri, SQLite, HTTP, or operating-system APIs.
pub mod error;
pub mod job;
pub mod platform;
