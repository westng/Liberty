#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobExecutionEventType {
    Created,
    Queued,
    Started,
    Progress,
    Completed,
    Failed,
    CancelRequested,
    Cancelled,
    Retried,
    Resumed,
}

impl JobExecutionStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobExecutionStatus::Completed
                | JobExecutionStatus::Failed
                | JobExecutionStatus::Cancelled
        )
    }
}
