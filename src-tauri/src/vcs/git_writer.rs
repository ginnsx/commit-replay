use crate::{
    error::Result,
    model::{ApplyResult, ApplyStatus, ReplayUnitMeta, ChangeSet, ValidationResult},
};
use super::VcsWriter;

pub struct GitWriter {
    pub repo_path: String,
}

impl VcsWriter for GitWriter {
    fn prepare(&self, _branch: &str) -> Result<String> {
        // TODO(M3): run `git status --porcelain`, check clean, return HEAD SHA
        Ok(String::new())
    }

    fn apply(&self, _changeset: &ChangeSet) -> Result<ApplyResult> {
        // TODO(M3): apply FileChanges to working copy
        Ok(ApplyResult {
            status: ApplyStatus::Ok,
            message: None,
            failed_paths: vec![],
        })
    }

    fn validate(&self) -> Result<ValidationResult> {
        // TODO(M4): scan for conflict markers, run custom script
        Ok(ValidationResult { passed: true, issues: vec![] })
    }

    fn commit(&self, _meta: &ReplayUnitMeta, _message_template: &str) -> Result<String> {
        // TODO(M3): `git add -A` + `git commit -m ...`
        Ok(String::new())
    }

    fn rollback(&self, _checkpoint: &str) -> Result<()> {
        // TODO(M4): `git reset --hard {checkpoint}`
        Ok(())
    }
}
