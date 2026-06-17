use crate::{
    error::{AppError, Result},
    model::{ChangeSet, ReplayUnitMeta},
};

use super::VcsReader;
use super::svn::{
    parse_log_xml, parse_svn_revision, parse_unified_diff, svn_cat_file, svn_diff_revision,
    svn_log_revision_xml, svn_log_xml_paged,
    SvnCredentials,
};

#[derive(Clone)]
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

    pub fn list_recent_paged(
        &self,
        limit: usize,
        before_revision: Option<u64>,
    ) -> Result<Vec<ReplayUnitMeta>> {
        svn_log_xml_paged(&self.wc_path, &self.creds(), limit, before_revision)
    }
}

impl VcsReader for SvnReader {
    fn list_recent(&self, limit: usize) -> Result<Vec<ReplayUnitMeta>> {
        self.list_recent_paged(limit, None)
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
    let mut files = parse_unified_diff(&diff, Some(&reader.wc_path))?;
    tag_source_ref(&mut files, &meta.source_ref);
    Ok(ChangeSet {
        meta: ReplayUnitMeta {
            changed_paths_count: files.len(),
            ..meta
        },
        files,
    })
}

pub fn tag_source_ref(files: &mut [crate::model::FileChange], source_ref: &str) {
    for fc in files {
        fc.source_ref = Some(source_ref.to_string());
    }
}

pub fn attach_source_after_file(
    reader: &SvnReader,
    revision: u64,
    fc: &mut crate::model::FileChange,
) {
    use crate::model::FileChangeKind;
    use std::path::Path;

    if fc.source_after.is_some() {
        return;
    }
    if matches!(fc.kind, FileChangeKind::Delete | FileChangeKind::Binary) {
        return;
    }
    let rel = fc.path.trim_start_matches('/').replace('/', std::path::MAIN_SEPARATOR_STR);
    let file_path = Path::new(&reader.wc_path).join(rel);
    if let Ok(content) = svn_cat_file(&reader.creds(), revision, &file_path.to_string_lossy()) {
        fc.source_after = Some(content);
    }
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
