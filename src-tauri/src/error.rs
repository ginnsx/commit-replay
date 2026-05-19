use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("VCS error: {0}")]
    Vcs(String),

    #[error("Mapping error: {0}")]
    Mapping(String),

    #[error("Apply failed: {0}")]
    Apply(String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Tauri commands must return a serialisable error type.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_serialises_to_string() {
        let err = AppError::Vcs("connection refused".into());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("connection refused"));
    }
}
