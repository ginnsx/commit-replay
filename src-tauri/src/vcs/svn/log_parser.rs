use quick_xml::de::from_str;
use serde::Deserialize;

use crate::{
    error::{AppError, Result},
    model::ReplayUnitMeta,
};

use super::ref_util::format_svn_ref;

#[derive(Debug, Deserialize)]
struct LogRoot {
    #[serde(rename = "logentry", default)]
    logentry: Vec<LogEntry>,
}

#[derive(Debug, Deserialize)]
struct LogEntry {
    #[serde(rename = "@revision")]
    revision: String,
    author: Option<String>,
    date: Option<String>,
    msg: Option<String>,
    paths: Option<Paths>,
}

#[derive(Debug, Deserialize)]
struct Paths {
    #[serde(rename = "path", default)]
    path: Vec<PathEntry>,
}

#[derive(Debug, Deserialize)]
struct PathEntry {
    #[serde(rename = "@action")]
    _action: Option<String>,
    #[serde(rename = "$text")]
    _text: String,
}

/// Parse `svn log --xml` output into commit metadata list (newest first, as SVN returns).
pub fn parse_log_xml(xml: &str) -> Result<Vec<ReplayUnitMeta>> {
    let root: LogRoot = from_str(xml)
        .map_err(|e| AppError::Vcs(format!("failed to parse svn log XML: {e}")))?;

    let mut entries = Vec::with_capacity(root.logentry.len());
    for entry in root.logentry {
        let revision = entry.revision.parse::<u64>().map_err(|e| {
            AppError::Vcs(format!("invalid revision in log XML: {e}"))
        })?;

        let changed_paths_count = entry
            .paths
            .as_ref()
            .map(|p| p.path.len())
            .unwrap_or(0);

        entries.push(ReplayUnitMeta {
            source_ref: format_svn_ref(revision),
            author: entry.author.unwrap_or_default(),
            date: entry.date.unwrap_or_default(),
            message: entry.msg.unwrap_or_default().trim().to_string(),
            changed_paths_count,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/svn")
            .join(name);
        std::fs::read_to_string(path).expect("read fixture")
    }

    #[test]
    fn parses_sample_log() {
        let entries = parse_log_xml(&fixture("log_sample.xml")).unwrap();
        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].source_ref, "svn:100");
        assert_eq!(entries[0].author, "alice");
        assert_eq!(entries[0].changed_paths_count, 2);
        assert!(entries[0].message.contains("Initial import"));

        assert_eq!(entries[1].source_ref, "svn:101");
        assert_eq!(entries[1].author, "bob");
        assert_eq!(entries[1].changed_paths_count, 1);

        assert_eq!(entries[2].source_ref, "svn:102");
        assert_eq!(entries[2].changed_paths_count, 1);
    }

    #[test]
    fn rejects_invalid_xml() {
        assert!(parse_log_xml("not valid xml <<").is_err());
    }
}
