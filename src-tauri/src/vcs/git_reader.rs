use crate::{
    error::{AppError, Result},
    model::{ChangeSet, ReplayUnitMeta},
    vcs::svn::parse_unified_diff,
};

use super::git::{log_parser::parse_git_log, ref_util::parse_git_revision};
use super::svn_reader::tag_source_ref;
use super::VcsReader;

#[derive(Clone)]
pub struct GitReader {
    pub repo_path: String,
}

fn decode_git_text_output(bytes: &[u8]) -> String {
    let mut output = String::new();
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if let Ok(text) = std::str::from_utf8(line) {
            output.push_str(text);
            continue;
        }
        let (decoded, _, had_errors) = encoding_rs::GBK.decode(line);
        if had_errors {
            output.push_str(&String::from_utf8_lossy(line));
        } else {
            output.push_str(&decoded);
        }
    }
    output
}

impl GitReader {
    fn run_git_output(&self, args: &[&str]) -> Result<std::process::Output> {
        let mut git_args = vec!["-c", "core.quotePath=false"];
        git_args.extend_from_slice(args);
        let output = crate::process::output(
            crate::process::command("git")
                .current_dir(&self.repo_path)
                .args(&git_args),
        )
        .map_err(|e| AppError::Vcs(format!("failed to spawn git: {e}")))?;
        if output.status.success() {
            return Ok(output);
        }
        let stdout = decode_git_text_output(&output.stdout);
        let stderr = decode_git_text_output(&output.stderr);
        Err(AppError::Vcs(format!(
            "git {} failed: {stderr}{stdout}",
            args.first().copied().unwrap_or("")
        )))
    }

    fn run_git(&self, args: &[&str]) -> Result<String> {
        self.run_git_output(args)
            .map(|output| decode_git_text_output(&output.stdout))
    }

    fn run_git_bytes(&self, args: &[&str]) -> Result<Vec<u8>> {
        self.run_git_output(args).map(|output| output.stdout)
    }

    pub fn list_recent_paged(
        &self,
        limit: usize,
        before_cursor: Option<&str>,
    ) -> Result<Vec<ReplayUnitMeta>> {
        let format = "%H%x00%an%x00%aI%x00%s%x00";
        let limit_s = limit.to_string();
        let output = match before_cursor {
            None => self.run_git(&[
                "log",
                &format!("--format={format}"),
                "--numstat",
                "-n",
                &limit_s,
            ])?,
            Some(cursor) => {
                let sha = parse_git_revision(cursor)?;
                let range = format!("{sha}^");
                self.run_git(&[
                    "log",
                    &format!("--format={format}"),
                    "--numstat",
                    "-n",
                    &limit_s,
                    &range,
                ])?
            }
        };
        let entries = parse_git_log(&output)?;
        Ok(entries)
    }
}

impl VcsReader for GitReader {
    fn list_recent(&self, limit: usize) -> Result<Vec<ReplayUnitMeta>> {
        self.list_recent_paged(limit, None)
    }

    fn load_changeset(&self, source_ref: &str) -> Result<ChangeSet> {
        let sha = parse_git_revision(source_ref)?;
        let meta = self.meta_for_commit(&sha)?;
        load_changeset_with_meta(self, meta)
    }
}

impl GitReader {
    fn meta_for_commit(&self, sha: &str) -> Result<ReplayUnitMeta> {
        let format = "%H%x00%an%x00%aI%x00%s%x00";
        let output = self.run_git(&[
            "log",
            &format!("--format={format}"),
            "--numstat",
            "-n",
            "1",
            sha,
        ])?;
        let entries = parse_git_log(&output)?;
        entries
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Vcs(format!("no log entry for commit {sha}")))
    }
}

pub fn load_changeset_with_meta(reader: &GitReader, meta: ReplayUnitMeta) -> Result<ChangeSet> {
    let sha = parse_git_revision(&meta.source_ref)?;
    let diff = reader.run_git(&["show", "--format=", "--patch", &sha])?;
    let mut files = parse_unified_diff(&diff, Some(&reader.repo_path))?;
    tag_source_ref(&mut files, &meta.source_ref);

    for fc in &mut files {
        if !matches!(fc.kind, crate::model::FileChangeKind::Delete) {
            let blob_path = fc.path.trim_start_matches('/');
            let object = format!("{sha}:{blob_path}");
            if matches!(fc.kind, crate::model::FileChangeKind::Binary) {
                if let Ok(content) = reader.run_git_bytes(&["show", &object]) {
                    fc.after_bytes = Some(content);
                }
            } else if let Ok(content) = reader.run_git(&["show", &object]) {
                fc.source_after = Some(content.clone());
                if (fc.kind == crate::model::FileChangeKind::Add && fc.patch.is_none())
                    || (fc.kind == crate::model::FileChangeKind::Rename && fc.patch.is_none())
                {
                    fc.after = Some(content);
                }
            }
        }
    }

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
    fn reader_stores_repo_path() {
        let reader = GitReader {
            repo_path: "C:\\repo".into(),
        };
        assert_eq!(reader.repo_path, "C:\\repo");
    }

    #[test]
    fn decodes_utf8_paths_and_gbk_patch_content_independently() {
        let mut output = "diff --git a/中文路径.sql b/中文路径.sql\n".as_bytes().to_vec();
        let (gbk_content, _, had_errors) = encoding_rs::GBK.encode("+中文内容\n");
        assert!(!had_errors);
        output.extend_from_slice(&gbk_content);

        assert!(std::str::from_utf8(&output).is_err());
        let decoded = decode_git_text_output(&output);
        assert!(decoded.contains("a/中文路径.sql"));
        assert!(decoded.contains("+中文内容\n"));
        assert!(!decoded.contains('\u{fffd}'));
    }
}
