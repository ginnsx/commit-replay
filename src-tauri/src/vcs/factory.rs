use crate::error::{AppError, Result};
use crate::model::{ApplyResult, ChangeSet, ReplayUnitMeta};
use crate::store::models::{IntegrationStrategy, RepoRecord, RepoType};

use super::git_reader::GitReader;
use super::git_writer::GitWriter;
use super::svn_reader::SvnReader;
use super::svn_writer::SvnWriter;
use super::VcsReader;
use super::VcsWriter;

pub enum SourceReader {
    Svn(SvnReader),
    Git(GitReader),
}

impl SourceReader {
    pub fn from_repo(repo: &RepoRecord, password: Option<String>) -> Result<Self> {
        match repo.repo_type {
            RepoType::Svn => Ok(SourceReader::Svn(SvnReader {
                wc_path: repo.path.clone(),
                username: repo.svn_user.clone(),
                password,
            })),
            RepoType::Git => Ok(SourceReader::Git(GitReader {
                repo_path: repo.path.clone(),
            })),
        }
    }

    pub fn list_recent_paged(
        &self,
        limit: usize,
        before_cursor: Option<&str>,
    ) -> Result<Vec<ReplayUnitMeta>> {
        match self {
            SourceReader::Svn(r) => {
                let before_revision = before_cursor.and_then(|c| {
                    crate::vcs::svn::ref_util::parse_svn_revision(c).ok()
                });
                r.list_recent_paged(limit, before_revision)
            }
            SourceReader::Git(r) => r.list_recent_paged(limit, before_cursor),
        }
    }

    pub fn load_changeset(&self, source_ref: &str) -> Result<ChangeSet> {
        match self {
            SourceReader::Svn(r) => r.load_changeset(source_ref),
            SourceReader::Git(r) => r.load_changeset(source_ref),
        }
    }
}

pub enum MigrationWriter {
    Git(GitWriter),
    Svn(SvnWriter),
}

impl MigrationWriter {
    pub fn from_repo(repo: &RepoRecord, password: Option<String>) -> Result<Self> {
        match repo.repo_type {
            RepoType::Git => Ok(MigrationWriter::Git(GitWriter {
                repo_path: repo.path.clone(),
            })),
            RepoType::Svn => Ok(MigrationWriter::Svn(SvnWriter {
                wc_path: repo.path.clone(),
                username: repo.svn_user.clone(),
                password,
            })),
        }
    }

    pub fn prepare(&self, branch: &str) -> Result<String> {
        match self {
            MigrationWriter::Git(w) => w.prepare(branch),
            MigrationWriter::Svn(w) => w.prepare(branch),
        }
    }

    pub fn apply_changeset_with_strategies(
        &self,
        changeset: &ChangeSet,
        strategies: &std::collections::HashMap<String, IntegrationStrategy>,
    ) -> Result<ApplyResult> {
        match self {
            MigrationWriter::Git(w) => w.apply_changeset_with_strategies(changeset, strategies),
            MigrationWriter::Svn(w) => w.apply_changeset_with_strategies(changeset, strategies),
        }
    }

    pub fn commit(&self, meta: &ReplayUnitMeta, message_template: &str) -> Result<String> {
        match self {
            MigrationWriter::Git(w) => w.commit(meta, message_template),
            MigrationWriter::Svn(w) => w.commit(meta, message_template),
        }
    }

    pub fn rollback(&self, checkpoint: &str) -> Result<()> {
        match self {
            MigrationWriter::Git(w) => w.rollback(checkpoint),
            MigrationWriter::Svn(w) => w.rollback(checkpoint),
        }
    }
}

pub fn repo_type_str(repo_type: &RepoType) -> &'static str {
    match repo_type {
        RepoType::Git => "git",
        RepoType::Svn => "svn",
    }
}

pub fn ensure_different_repos(source: &RepoRecord, target: &RepoRecord) -> Result<()> {
    if source.id == target.id {
        return Err(AppError::Validation("源仓库与目标仓库不能相同".into()));
    }
    Ok(())
}
