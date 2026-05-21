use crate::{
    error::{AppError, Result},
    model::{ChangeSet, ReplayUnitMeta},
};

use super::VcsReader;
use super::svn::{
    SvnCredentials, parse_log_xml, parse_svn_revision, parse_unified_diff,
    svn_diff_revision, svn_log_revision_xml, svn_log_xml,
};

pub struct SvnReader {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl SvnReader {
    fn creds(&self) -> SvnCredentials {
        SvnCredentials {
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }

    fn meta_for_revision(&self, revision: u64) -> Result<ReplayUnitMeta> {
        let xml = svn_log_revision_xml(&self.url, &self.creds(), revision)?;
        let entries = parse_log_xml(&xml)?;
        entries.into_iter().next().ok_or_else(|| {
            AppError::Vcs(format!("no log entry for revision {revision}"))
        })
    }
}

impl VcsReader for SvnReader {
    fn list_recent(&self, limit: usize) -> Result<Vec<ReplayUnitMeta>> {
        let xml = svn_log_xml(&self.url, &self.creds(), limit)?;
        parse_log_xml(&xml)
    }

    fn load_changeset(&self, source_ref: &str) -> Result<ChangeSet> {
        let revision = parse_svn_revision(source_ref)?;
        let meta = self.meta_for_revision(revision)?;
        load_changeset_with_meta(self, meta)
    }
}

/// Load file changes for a commit whose metadata is already known (avoids extra log fetch).
pub fn load_changeset_with_meta(reader: &SvnReader, meta: ReplayUnitMeta) -> Result<ChangeSet> {
    let revision = parse_svn_revision(&meta.source_ref)?;
    let diff = svn_diff_revision(&reader.url, &reader.creds(), revision)?;
    let files = parse_unified_diff(&diff)?;
    Ok(ChangeSet {
        meta: ReplayUnitMeta {
            changed_paths_count: files.len(),
            ..meta
        },
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_stores_connection_fields() {
        let reader = SvnReader {
            url: "https://svn.example.com/repo".into(),
            username: Some("u".into()),
            password: Some("p".into()),
        };
        assert_eq!(reader.creds().username.as_deref(), Some("u"));
    }
}
