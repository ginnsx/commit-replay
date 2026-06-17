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

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Serialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl AppError {
    fn payload(&self) -> ErrorPayload {
        match self {
            Self::Vcs(m) => ErrorPayload {
                code: "vcs".into(),
                message: m.clone(),
                retryable: true,
            },
            Self::Auth(m) => ErrorPayload {
                code: "auth".into(),
                message: m.clone(),
                retryable: false,
            },
            Self::Mapping(m) => ErrorPayload {
                code: "mapping".into(),
                message: m.clone(),
                retryable: false,
            },
            Self::Apply(m) => ErrorPayload {
                code: "apply".into(),
                message: m.clone(),
                retryable: false,
            },
            Self::Validation(m) => ErrorPayload {
                code: "validation".into(),
                message: m.clone(),
                retryable: false,
            },
            Self::Io(e) => ErrorPayload {
                code: "io".into(),
                message: e.to_string(),
                retryable: false,
            },
            Self::Other(e) => ErrorPayload {
                code: "other".into(),
                message: e.to_string(),
                retryable: false,
            },
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.payload().serialize(serializer)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_serialises_payload() {
        let err = AppError::Vcs("connection refused".into());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("connection refused"));
        assert!(json.contains("vcs"));
    }
}
