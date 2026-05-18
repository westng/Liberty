//! Infrastructure integrations.
//!
//! SQLite repositories, file-system adapters, Python process runners, HTTP
//! clients, credential stores, and platform-specific code belong here.
pub mod credentials;
pub mod ids;
pub mod migrations;
pub mod process_logs;
pub mod repositories;
pub mod runner_files;
pub mod runner_process;
pub mod time;
