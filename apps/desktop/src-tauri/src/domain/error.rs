use std::{collections::BTreeMap, fmt};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Validation,
    NotFound,
    Conflict,
    Authorization,
    Network,
    Credentials,
    Runtime,
    Protocol,
    Infrastructure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub code: String,
    pub category: ErrorCategory,
    pub retryable: bool,
    pub params: BTreeMap<String, String>,
}

impl AppErrorDto {
    pub fn new(code: impl Into<String>, category: ErrorCategory, retryable: bool) -> Self {
        Self {
            code: code.into(),
            category,
            retryable,
            params: BTreeMap::new(),
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}

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

    pub fn into_dto(self) -> AppErrorDto {
        match self {
            AppError::Validation(_) => {
                AppErrorDto::new("validation_error", ErrorCategory::Validation, false)
            }
            AppError::NotFound(_) => AppErrorDto::new("not_found", ErrorCategory::NotFound, false),
            AppError::Conflict(_) => AppErrorDto::new("conflict", ErrorCategory::Conflict, true),
            AppError::Infrastructure(_) => {
                AppErrorDto::new("infrastructure_error", ErrorCategory::Infrastructure, true)
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_error_excludes_internal_message() {
        let serialized = serde_json::to_string(
            &AppError::Infrastructure("secret provider response".into()).into_dto(),
        )
        .expect("serialize error");

        assert!(serialized.contains("infrastructure_error"));
        assert!(!serialized.contains("secret provider response"));
    }
}
