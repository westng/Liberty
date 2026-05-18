use std::fmt;

#[derive(Debug, Clone)]
pub enum AppError {
    Validation(String),
    NotFound(String),
    Conflict(String),
    Infrastructure(String),
}

impl AppError {
    pub fn user_message(&self) -> String {
        match self {
            AppError::Validation(message)
            | AppError::NotFound(message)
            | AppError::Conflict(message)
            | AppError::Infrastructure(message) => message.clone(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.user_message())
    }
}

impl std::error::Error for AppError {}

impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.user_message()
    }
}
