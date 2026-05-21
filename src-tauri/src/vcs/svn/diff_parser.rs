use crate::{
    error::Result,
    model::{FileChange, FileChangeKind},
};

/// Parse `svn diff -c REV` unified diff output into file changes.
pub fn parse_unified_diff(diff: &str) -> Result<Vec<FileChange>> {
    let mut files = Vec::new();
    let mut current_index_path: Option<String> = None;
    let mut current_patch = String::new();
    let mut old_path: Option<String> = None;
    let mut new_path: Option<String> = None;
    let mut is_binary = false;

    let flush = |files: &mut Vec<FileChange>,
                 index_path: &mut Option<String>,
                 patch: &mut String,
                 old: &mut Option<String>,
                 new: &mut Option<String>,
                 binary: &mut bool| {
        if *binary {
            if let Some(path) = index_path.take().or_else(|| new.clone()).or_else(|| old.clone()) {
                files.push(FileChange {
                    path: normalize_svn_path(&path),
                    target_path: None,
                    kind: FileChangeKind::Binary,
                    old_path: None,
                    before: None,
                    after: None,
                    patch: None,
                    conflict_risk: None,
                });
            }
        } else if let Some(path) = new
            .clone()
            .or_else(|| old.clone())
            .or_else(|| index_path.clone())
        {
            let path = normalize_svn_path(&path);
            let kind = infer_kind(old.as_deref(), new.as_deref(), patch);
            let patch_body = if patch.is_empty() { None } else { Some(patch.clone()) };
            files.push(FileChange {
                path: path.clone(),
                target_path: None,
                kind,
                old_path: old.as_ref().map(|p| normalize_svn_path(p)),
                before: None,
                after: None,
                patch: patch_body,
                conflict_risk: None,
            });
        }

        *index_path = None;
        patch.clear();
        *old = None;
        *new = None;
        *binary = false;
    };

    for line in diff.lines() {
        if line.starts_with("Index: ") {
            flush(
                &mut files,
                &mut current_index_path,
                &mut current_patch,
                &mut old_path,
                &mut new_path,
                &mut is_binary,
            );
            current_index_path = Some(line["Index: ".len()..].trim().to_string());
            continue;
        }

        if line.contains("Cannot display: file marked as a binary type") {
            is_binary = true;
            continue;
        }

        if line.starts_with("--- ") {
            old_path = Some(parse_diff_path_line(line, "--- "));
            continue;
        }

        if line.starts_with("+++ ") {
            new_path = Some(parse_diff_path_line(line, "+++ "));
            continue;
        }

        if line.starts_with("diff --git ") {
            continue;
        }

        if line.starts_with("Property changes on: ") || line.starts_with("___") {
            continue;
        }

        if line.starts_with("@@") || !line.is_empty() || !current_patch.is_empty() {
            if old_path.is_some() || new_path.is_some() || current_index_path.is_some() {
                current_patch.push_str(line);
                current_patch.push('\n');
            }
        }
    }

    flush(
        &mut files,
        &mut current_index_path,
        &mut current_patch,
        &mut old_path,
        &mut new_path,
        &mut is_binary,
    );

    Ok(files)
}

fn parse_diff_path_line(line: &str, prefix: &str) -> String {
    let rest = line.strip_prefix(prefix).unwrap_or(line).trim();
    let path = rest.split('\t').next().unwrap_or(rest).trim();
    path.to_string()
}

fn normalize_svn_path(path: &str) -> String {
    let p = path.trim();
    if p.starts_with('/') {
        p.to_string()
    } else {
        format!("/{p}")
    }
}

fn infer_kind(old: Option<&str>, new: Option<&str>, patch: &str) -> FileChangeKind {
    if let Some(hunk) = patch.lines().find(|l| l.starts_with("@@")) {
        if hunk.contains("-0,0") || hunk.starts_with("@@ -0,") {
            return FileChangeKind::Add;
        }
        if hunk.contains("+0,0") {
            return FileChangeKind::Delete;
        }
    }

    let old_norm = old.map(normalize_svn_path);
    let new_norm = new.map(|p| normalize_svn_path(p));
    if let (Some(o), Some(n)) = (old_norm.as_deref(), new_norm.as_deref()) {
        if o != n {
            return FileChangeKind::Rename;
        }
    }

    FileChangeKind::Modify
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/svn")
            .join(name);
        std::fs::read_to_string(path).expect("read fixture")
    }

    #[test]
    fn parses_modify_diff() {
        let files = parse_unified_diff(&fixture("diff_modify.txt")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/trunk/README.md");
        assert_eq!(files[0].kind, FileChangeKind::Modify);
        assert!(files[0].patch.as_ref().is_some_and(|p| p.contains("@@")));
    }

    #[test]
    fn parses_add_and_delete() {
        let files = parse_unified_diff(&fixture("diff_add_delete.txt")).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "/trunk/new-file.txt");
        assert_eq!(files[0].kind, FileChangeKind::Add);
        assert_eq!(files[1].path, "/trunk/remove-me.txt");
        assert_eq!(files[1].kind, FileChangeKind::Delete);
    }

    #[test]
    fn parses_binary_diff() {
        let files = parse_unified_diff(&fixture("diff_binary.txt")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/trunk/assets/logo.png");
        assert_eq!(files[0].kind, FileChangeKind::Binary);
        assert!(files[0].patch.is_none());
    }

    #[test]
    fn empty_diff_returns_empty_vec() {
        assert!(parse_unified_diff("").unwrap().is_empty());
    }
}
