use crate::{
    error::{AppError, Result},
    model::{ChangeSet, ReplayUnitMeta},
};

use super::VcsReader;
use super::svn::{
    parse_log_xml, parse_svn_revision, parse_unified_diff, svn_diff_revision,
    svn_log_revision_xml, svn_log_xml_paged,
    SvnCredentials,
};

pub struct SvnReader {
    pub wc_path: String,
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
        let xml = svn_log_revision_xml(&self.wc_path, &self.creds(), revision)?;
        let entries = parse_log_xml(&xml)?;
        entries.into_iter().next().ok_or_else(|| {
            AppError::Vcs(format!("no log entry for revision {revision}"))
        })
    }

    pub fn list_recent_paged(&self, limit: usize, offset: usize) -> Result<Vec<ReplayUnitMeta>> {
        svn_log_xml_paged(&self.wc_path, &self.creds(), limit, offset)
    }
}

impl VcsReader for SvnReader {
    fn list_recent(&self, limit: usize) -> Result<Vec<ReplayUnitMeta>> {
        self.list_recent_paged(limit, 0)
    }

    fn load_changeset(&self, source_ref: &str) -> Result<ChangeSet> {
        let revision = parse_svn_revision(source_ref)?;
        let meta = self.meta_for_revision(revision)?;
        load_changeset_with_meta(self, meta)
    }
}

pub fn load_changeset_with_meta(reader: &SvnReader, meta: ReplayUnitMeta) -> Result<ChangeSet> {
    let revision = parse_svn_revision(&meta.source_ref)?;
    let diff = svn_diff_revision(&reader.wc_path, &reader.creds(), revision)?;
    let files = parse_unified_diff(&diff, Some(&reader.wc_path))?;
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
            wc_path: "C:\\svn\\wc".into(),
            username: Some("u".into()),
            password: Some("p".into()),
        };
        assert_eq!(reader.creds().username.as_deref(), Some("u"));
    }
}
