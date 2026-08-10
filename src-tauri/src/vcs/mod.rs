pub mod factory;
pub mod git;
pub mod git_probe;
pub mod git_reader;
pub mod git_writer;

pub use factory::{ensure_different_repos, repo_type_str, MigrationWriter, SourceReader};
pub use git_probe::{probe_git_repo, GitRepoInfo};
pub mod svn;
pub mod svn_reader;
pub mod svn_writer;

use crate::{error::Result, model::{ChangeSet, ReplayUnitMeta}};

#[derive(Debug, Clone)]
pub struct VcsCheckpoint {
    pub reference: String,
    pub base_branch: Option<String>,
}

/// Source VCS: list recent commits and load a single commit's ChangeSet.
pub trait VcsReader: Send + Sync {
    fn list_recent(&self, limit: usize) -> Result<Vec<ReplayUnitMeta>>;
    fn load_changeset(&self, source_ref: &str) -> Result<ChangeSet>;
}

/// Target VCS: apply and commit a ChangeSet.
pub trait VcsWriter: Send + Sync {
    /// Ensure the working copy is clean and record a checkpoint to roll back to.
    fn prepare(&self, branch: &str) -> Result<VcsCheckpoint>;
    /// Apply file changes from a ChangeSet to the working copy.
    fn apply(&self, changeset: &ChangeSet) -> Result<crate::model::ApplyResult>;
    /// Validate the working copy (conflict markers, missing files, custom script).
    fn validate(&self) -> Result<crate::model::ValidationResult>;
    /// Commit the staged changes and return the new VCS ref.
    fn commit(&self, meta: &ReplayUnitMeta, message_template: &str) -> Result<String>;
    /// Roll back the working copy to the checkpoint returned by `prepare`.
    fn rollback(&self, checkpoint: &VcsCheckpoint, created_branch: Option<&str>) -> Result<()>;
}
