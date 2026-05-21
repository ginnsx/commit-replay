use crate::error::{AppError, Result};

/// Parse `svn:12345` → `12345`.
pub fn parse_svn_revision(source_ref: &str) -> Result<u64> {
    let rev = source_ref
        .strip_prefix("svn:")
        .ok_or_else(|| AppError::Vcs(format!("invalid svn source_ref: {source_ref}")))?;
    rev.parse::<u64>()
        .map_err(|e| AppError::Vcs(format!("invalid svn revision '{rev}': {e}")))
}

pub fn format_svn_ref(revision: u64) -> String {
    format!("svn:{revision}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_ref() {
        assert_eq!(parse_svn_revision("svn:42").unwrap(), 42);
    }

    #[test]
    fn rejects_invalid_prefix() {
        assert!(parse_svn_revision("git:abc").is_err());
    }

    #[test]
    fn format_round_trip() {
        assert_eq!(format_svn_ref(101), "svn:101");
    }
}
