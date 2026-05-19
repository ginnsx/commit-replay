use crate::{
    error::Result,
    model::{ApplyResult, ApplyStatus, ReplayUnitMeta, ChangeSet, ValidationResult},
};
use super::VcsWriter;

pub struct SvnWriter {
    pub wc_path: String,
}

impl VcsWriter for SvnWriter {
    fn prepare(&self, _branch: &str) -> Result<String> {
        // TODO(future): `svn status`, return BASE revision
        Ok(String::new())
    }

    fn apply(&self, _changeset: &ChangeSet) -> Result<ApplyResult> {
        // TODO(future): apply FileChanges, use `patch` or file write
        Ok(ApplyResult {
            status: ApplyStatus::Ok,
            message: None,
            failed_paths: vec![],
        })
    }

    fn validate(&self) -> Result<ValidationResult> {
        Ok(ValidationResult { passed: true, issues: vec![] })
    }

    fn commit(&self, _meta: &ReplayUnitMeta, _message_template: &str) -> Result<String> {
        // TODO(future): `svn commit -m ...`
        Ok(String::new())
    }

    fn rollback(&self, _checkpoint: &str) -> Result<()> {
        // TODO(future): `svn revert -R .` + `svn update -r {checkpoint}`
        Ok(())
    }
}
