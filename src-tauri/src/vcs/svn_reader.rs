use crate::{error::Result, model::{ChangeSet, ReplayUnitMeta}};
use super::VcsReader;

pub struct SvnReader {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl VcsReader for SvnReader {
    fn list_recent(&self, _limit: usize) -> Result<Vec<ReplayUnitMeta>> {
        // TODO(M1): call `svn log --xml -l {limit}` and parse XML
        Ok(vec![])
    }

    fn load_changeset(&self, _source_ref: &str) -> Result<ChangeSet> {
        // TODO(M1): call `svn diff -c {rev}` and parse unified diff
        todo!("SvnReader::load_changeset")
    }
}
