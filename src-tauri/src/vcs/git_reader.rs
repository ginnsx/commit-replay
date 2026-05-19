use crate::{error::Result, model::{ChangeSet, ReplayUnitMeta}};
use super::VcsReader;

pub struct GitReader {
    pub repo_path: String,
}

impl VcsReader for GitReader {
    fn list_recent(&self, _limit: usize) -> Result<Vec<ReplayUnitMeta>> {
        // TODO(future): call `git log --format=...`
        Ok(vec![])
    }

    fn load_changeset(&self, _source_ref: &str) -> Result<ChangeSet> {
        // TODO(future): call `git show {sha}`
        todo!("GitReader::load_changeset")
    }
}
