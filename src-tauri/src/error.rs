use serde::Serialize;

/// Application-wide error type. Serialized to a structured payload so the
/// frontend can present categorized, actionable errors.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Launch(String),
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Provider(String),
    #[error("scan cancelled")]
    Cancelled,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorPayload<'a> {
    kind: &'a str,
    message: String,
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Db(_) => "database",
            AppError::Io(_) => "io",
            AppError::NotFound(_) => "notFound",
            AppError::Invalid(_) => "invalid",
            AppError::Launch(_) => "launch",
            AppError::Network(_) => "network",
            AppError::Provider(_) => "provider",
            AppError::Cancelled => "cancelled",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        ErrorPayload {
            kind: self.kind(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
