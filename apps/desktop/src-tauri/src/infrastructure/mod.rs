//! Infrastructure integrations.
//!
//! SQLite repositories, file-system adapters, Python process runners, HTTP
//! clients, credential stores, and platform-specific code belong here.
pub mod credentials;
pub mod ids;
pub mod ipc_contracts;
pub mod migrations;
pub mod network;
pub mod observability;
pub mod process_logs;
pub mod repositories;
pub mod runner_files;
pub mod runner_process;
pub mod runner_protocol;
pub mod time;
