use crate::{
    error::{AppError, Result},
    model::{ChangeSet, FileChange, FileChangeKind, ReplayUnitMeta},
};

use super::svn::{
    encoded_child_url, parse_diff_summary_xml, parse_log_xml, parse_svn_revision,
    parse_unified_diff, probe_svn_wc, svn_cat_file, svn_cat_file_bytes, svn_diff_file_revision,
    svn_diff_summary_xml, svn_log_revision_xml, svn_log_xml_paged, SvnCredentials,
    SvnDiffSummaryEntry,
};
use super::VcsReader;

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

    /// Repository URL for log queries (HEAD range). WC path limits log to BASE.
    fn repo_url(&self) -> Result<String> {
        probe_svn_wc(&self.wc_path).map(|info| info.url)
    }

    fn meta_for_revision(&self, revision: u64) -> Result<ReplayUnitMeta> {
        let url = self.repo_url()?;
        let xml = svn_log_revision_xml(&url, &self.creds(), revision)?;
        let entries = parse_log_xml(&xml)?;
        entries
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Vcs(format!("no log entry for revision {revision}")))
    }

    pub fn list_recent_paged(
        &self,
        limit: usize,
        before_revision: Option<u64>,
    ) -> Result<Vec<ReplayUnitMeta>> {
        let url = self.repo_url()?;
        svn_log_xml_paged(&url, &self.creds(), limit, before_revision)
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
    let branch_url = reader.repo_url()?;
    let summary_xml = svn_diff_summary_xml(&branch_url, &reader.creds(), revision)?;
    let summary = parse_diff_summary_xml(&summary_xml, &branch_url)?;
    let mut files = Vec::with_capacity(summary.len());
    for entry in summary {
        files.push(load_summary_file(reader, revision, entry)?);
    }
    tag_source_ref(&mut files, &meta.source_ref);
    Ok(ChangeSet {
        meta: ReplayUnitMeta {
            changed_paths_count: files.len(),
            ..meta
        },
        files,
    })
}

fn load_summary_file(
    reader: &SvnReader,
    revision: u64,
    entry: SvnDiffSummaryEntry,
) -> Result<FileChange> {
    let summary_kind = entry.kind.clone();
    let peg_revision = if matches!(summary_kind, FileChangeKind::Delete) {
        revision.saturating_sub(1).max(1)
    } else {
        revision
    };
    let diff = svn_diff_file_revision(&entry.url, &reader.creds(), revision, peg_revision)?;
    let mut parsed = parse_unified_diff(&diff, None)?;
    let mut file = parsed.pop().unwrap_or_else(|| FileChange {
        path: entry.path.clone(),
        target_path: None,
        kind: entry.kind.clone(),
        old_path: None,
        before: None,
        after: None,
        source_after: None,
        after_bytes: None,
        source_ref: None,
        patch: None,
        conflict_risk: None,
        analysis: None,
    });
    if matches!(summary_kind, FileChangeKind::Delete) {
        file.kind = FileChangeKind::Delete;
    } else if matches!(file.kind, FileChangeKind::Binary) {
        if let Ok(content) = svn_cat_file_bytes(&reader.creds(), revision, &entry.url) {
            match String::from_utf8(content) {
                Ok(text) => {
                    // SlikSVN 可能把含中文的 UTF-8 文本误报为二进制，以实际内容为准。
                    file.kind = summary_kind;
                    file.source_after = Some(text);
                }
                Err(error) => file.after_bytes = Some(error.into_bytes()),
            }
        }
    } else {
        file.kind = summary_kind;
    }
    file.path = entry.path;
    file.old_path = None;
    Ok(file)
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
    if fc.source_after.is_some() || fc.after_bytes.is_some() {
        return;
    }
    if matches!(fc.kind, FileChangeKind::Delete) {
        return;
    }
    let Ok(branch_url) = reader.repo_url() else {
        return;
    };
    let file_url = encoded_child_url(&branch_url, &fc.path);
    if matches!(fc.kind, FileChangeKind::Binary) {
        if let Ok(content) = svn_cat_file_bytes(&reader.creds(), revision, &file_url) {
            fc.after_bytes = Some(content);
        }
    } else if let Ok(content) = svn_cat_file(&reader.creds(), revision, &file_url) {
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
