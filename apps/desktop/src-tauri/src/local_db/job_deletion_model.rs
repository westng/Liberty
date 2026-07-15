use super::LocalResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobDeletionPhase {
    Prepared,
    Fenced,
    Trashed,
    DatabaseDeleted,
}

impl JobDeletionPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Fenced => "fenced",
            Self::Trashed => "trashed",
            Self::DatabaseDeleted => "database_deleted",
        }
    }

    pub(crate) fn from_str(value: &str) -> LocalResult<Self> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "fenced" => Ok(Self::Fenced),
            "trashed" => Ok(Self::Trashed),
            "database_deleted" => Ok(Self::DatabaseDeleted),
            _ => Err(format!("删除操作阶段无效: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobDeletionOperation {
    pub operation_id: String,
    pub job_id: String,
    pub trash_name: String,
    pub phase: JobDeletionPhase,
    pub runner_pid: Option<u32>,
    pub runner_process_identity: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub last_error: Option<String>,
}
