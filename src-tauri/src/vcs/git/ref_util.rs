use crate::error::{AppError, Result};

pub fn parse_git_revision(source_ref: &str) -> Result<String> {
    source_ref
        .strip_prefix("git:")
        .map(str::to_string)
        .ok_or_else(|| AppError::Vcs(format!("invalid git source_ref: {source_ref}")))
}

pub fn format_git_ref(sha: &str) -> String {
    format!("git:{sha}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_ref() {
        assert_eq!(parse_git_revision("git:abc123").unwrap(), "abc123");
    }

    #[test]
    fn rejects_invalid_prefix() {
        assert!(parse_git_revision("svn:42").is_err());
    }
}
