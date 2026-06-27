use crate::{
    error::Result,
    model::{FileChange, FileChangeKind},
};

/// Parse `svn diff -c REV` unified diff output into file changes.
pub fn parse_unified_diff(diff: &str, wc_root: Option<&str>) -> Result<Vec<FileChange>> {
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
        let norm = |p: &str| normalize_svn_path(p, wc_root);
        if *binary {
            if let Some(path) = index_path
                .take()
                .or_else(|| new.clone())
                .or_else(|| old.clone())
            {
                files.push(FileChange {
                    path: norm(&path),
                    target_path: None,
                    kind: FileChangeKind::Binary,
                    old_path: None,
                    before: None,
                    after: None,
                    source_after: None,
                    source_ref: None,
                    patch: None,
                    conflict_risk: None,
                });
            }
        } else if let Some(path) = preferred_change_path(new.as_deref(), old.as_deref())
            .map(str::to_string)
            .or_else(|| index_path.clone())
        {
            let path = norm(&path);
            let kind = infer_kind(old.as_deref(), new.as_deref(), patch, wc_root);
            let patch_body = if patch.is_empty() {
                None
            } else {
                Some(patch.clone())
            };
            files.push(FileChange {
                path: path.clone(),
                target_path: None,
                kind,
                old_path: old.as_ref().map(|p| norm(p)),
                before: None,
                after: None,
                source_after: None,
                source_ref: None,
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
            flush(
                &mut files,
                &mut current_index_path,
                &mut current_patch,
                &mut old_path,
                &mut new_path,
                &mut is_binary,
            );
            if let Some((old, new)) = parse_git_diff_header(line) {
                old_path = Some(old);
                new_path = Some(new);
            }
            continue;
        }

        if line.starts_with("rename from ") {
            old_path = Some(line["rename from ".len()..].trim().to_string());
            continue;
        }

        if line.starts_with("rename to ") {
            new_path = Some(line["rename to ".len()..].trim().to_string());
            continue;
        }

        if line.starts_with("new file mode ") {
            old_path = Some("/dev/null".into());
            continue;
        }

        if line.starts_with("deleted file mode ") {
            new_path = Some("/dev/null".into());
            continue;
        }

        if line.starts_with("Binary files ") {
            is_binary = true;
            continue;
        }

        if line.starts_with("Property changes on: ") || line.starts_with("___") {
            continue;
        }

        if line.starts_with("@@") || !current_patch.is_empty() {
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
    strip_git_diff_prefix(path).to_string()
}

fn parse_git_diff_header(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("diff --git ")?;
    let mut parts = rest.split_whitespace();
    let old = strip_git_diff_prefix(parts.next()?).to_string();
    let new = strip_git_diff_prefix(parts.next()?).to_string();
    Some((old, new))
}

fn strip_git_diff_prefix(path: &str) -> &str {
    if path == "/dev/null" {
        return path;
    }
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

fn preferred_change_path<'a>(new: Option<&'a str>, old: Option<&'a str>) -> Option<&'a str> {
    match (new, old) {
        (Some("/dev/null"), Some(old_path)) => Some(old_path),
        (Some(new_path), _) => Some(new_path),
        (None, Some(old_path)) => Some(old_path),
        (None, None) => None,
    }
}

fn normalize_svn_path(path: &str, wc_root: Option<&str>) -> String {
    let p = path.trim().replace('\\', "/");
    if let Some(root) = wc_root.filter(|r| !r.trim().is_empty()) {
        let root_norm = root
            .trim()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase();
        let p_cmp = p.to_lowercase();
        if p_cmp.starts_with(&root_norm)
            && p_cmp
                .as_bytes()
                .get(root_norm.len())
                .is_none_or(|b| *b == b'/')
        {
            let suffix = p[root_norm.len()..].trim_start_matches('/');
            return if suffix.is_empty() {
                "/".into()
            } else {
                format!("/{suffix}")
            };
        }
    }
    if p.starts_with('/') {
        p
    } else {
        format!("/{p}")
    }
}

fn infer_kind(
    old: Option<&str>,
    new: Option<&str>,
    patch: &str,
    wc_root: Option<&str>,
) -> FileChangeKind {
    if old == Some("/dev/null") {
        return FileChangeKind::Add;
    }
    if new == Some("/dev/null") {
        return FileChangeKind::Delete;
    }

    if let Some(hunk) = patch.lines().find(|l| l.starts_with("@@")) {
        if hunk.contains("-0,0") || hunk.starts_with("@@ -0,") {
            return FileChangeKind::Add;
        }
        if hunk.contains("+0,0") {
            return FileChangeKind::Delete;
        }
    }

    let old_norm = old.map(|p| normalize_svn_path(p, wc_root));
    let new_norm = new.map(|p| normalize_svn_path(p, wc_root));
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
        let files = parse_unified_diff(&fixture("diff_modify.txt"), None).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/trunk/README.md");
        assert_eq!(files[0].kind, FileChangeKind::Modify);
        assert!(files[0].patch.as_ref().is_some_and(|p| p.contains("@@")));
    }

    #[test]
    fn parses_add_and_delete() {
        let files = parse_unified_diff(&fixture("diff_add_delete.txt"), None).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "/trunk/new-file.txt");
        assert_eq!(files[0].kind, FileChangeKind::Add);
        assert_eq!(files[1].path, "/trunk/remove-me.txt");
        assert_eq!(files[1].kind, FileChangeKind::Delete);
    }

    #[test]
    fn parses_binary_diff() {
        let files = parse_unified_diff(&fixture("diff_binary.txt"), None).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/trunk/assets/logo.png");
        assert_eq!(files[0].kind, FileChangeKind::Binary);
        assert!(files[0].patch.is_none());
    }

    #[test]
    fn empty_diff_returns_empty_vec() {
        assert!(parse_unified_diff("", None).unwrap().is_empty());
    }

    #[test]
    fn strips_git_a_b_prefixes() {
        let diff = "diff --git a/src/main.rs b/src/main.rs\n\
--- a/src/main.rs\n\
+++ b/src/main.rs\n\
@@ -1 +1 @@\n\
-old\n\
+new\n";
        let files = parse_unified_diff(diff, Some("C:\\repo")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/src/main.rs");
    }

    #[test]
    fn uses_old_path_for_git_delete() {
        let diff = "diff --git a/src/remove.rs b/src/remove.rs\n\
--- a/src/remove.rs\n\
+++ /dev/null\n\
@@ -1 +0,0 @@\n\
-old\n";
        let files = parse_unified_diff(diff, Some("C:\\repo")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/src/remove.rs");
        assert_eq!(files[0].kind, FileChangeKind::Delete);
    }

    #[test]
    fn strips_working_copy_root_from_windows_paths() {
        let diff = "Index: C:/wc/project/src/main.rs\n\
--- C:/wc/project/src/main.rs\t(revision 1)\n\
+++ C:/wc/project/src/main.rs\t(revision 2)\n\
@@ -1 +1 @@\n\
-old\n\
+new\n";
        let files = parse_unified_diff(diff, Some("C:\\wc\\project")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/src/main.rs");
    }

    #[test]
    fn parses_git_pure_rename() {
        let diff = "diff --git a/src/move_me.txt b/moved/move_me.txt\n\
similarity index 100%\n\
rename from src/move_me.txt\n\
rename to moved/move_me.txt\n";
        let files = parse_unified_diff(diff, Some("C:\\repo")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/moved/move_me.txt");
        assert_eq!(files[0].old_path.as_deref(), Some("/src/move_me.txt"));
        assert_eq!(files[0].kind, FileChangeKind::Rename);
        assert!(files[0].patch.is_none());
    }

    #[test]
    fn parses_git_binary_add() {
        let diff = "diff --git a/assets/blob.bin b/assets/blob.bin\n\
new file mode 100644\n\
index 0000000..67baa4c\n\
Binary files /dev/null and b/assets/blob.bin differ\n";
        let files = parse_unified_diff(diff, Some("C:\\repo")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/assets/blob.bin");
        assert_eq!(files[0].kind, FileChangeKind::Binary);
        assert!(files[0].patch.is_none());
    }

    #[test]
    fn parses_git_empty_file_add() {
        let diff = "diff --git a/src/empty.txt b/src/empty.txt\n\
new file mode 100644\n\
index 0000000..e69de29\n\
--- /dev/null\n\
+++ b/src/empty.txt\n";
        let files = parse_unified_diff(diff, Some("C:\\repo")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/src/empty.txt");
        assert_eq!(files[0].kind, FileChangeKind::Add);
        assert!(files[0].patch.is_none());
    }
}
