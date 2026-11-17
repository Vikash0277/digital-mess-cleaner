//! Typed errors returned to the UI as `{ code, message }`.

use serde::ser::{Serialize, SerializeStruct, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Database(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Permission(String),
    #[error("{0}")]
    Cancelled(String),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io_error",
            AppError::Database(_) => "database_error",
            AppError::NotFound(_) => "not_found",
            AppError::Invalid(_) => "invalid_input",
            AppError::Permission(_) => "permission_denied",
            AppError::Cancelled(_) => "cancelled",
            AppError::Internal(_) => "internal_error",
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::PermissionDenied => AppError::Permission(format!(
                "Permission denied. Choose an accessible folder or grant access in system settings. ({value})"
            )),
            std::io::ErrorKind::NotFound => AppError::NotFound(format!("File or folder not found. ({value})")),
            _ => AppError::Io(value.to_string()),
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Database(value.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
