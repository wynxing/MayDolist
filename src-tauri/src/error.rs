use serde::{Serialize, Serializer};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorPayload<'a> {
    code: &'a str,
    message: String,
    recoverable: bool,
}

/// Unified application error, serialized as a plain message string so the
/// frontend can surface it directly.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("corrupt JSON at {path}")]
    CorruptFile { path: String },
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("github error: {0}")]
    Github(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (code, recoverable) = match self {
            Self::InvalidInput(_) => ("INVALID_INPUT", true),
            Self::NotFound(_) => ("NOT_FOUND", true),
            Self::CorruptFile { .. } => ("CORRUPT_FILE", true),
            Self::Github(_) => ("GITHUB_ERROR", true),
            Self::Storage(_) => ("STORAGE_ERROR", true),
            Self::Internal(_) => ("INTERNAL_ERROR", false),
        };
        ErrorPayload {
            code,
            message: self.to_string(),
            recoverable,
        }
        .serialize(serializer)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        AppError::Storage(value.to_string())
    }
}
