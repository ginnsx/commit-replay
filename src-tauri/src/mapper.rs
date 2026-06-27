use crate::{
    error::{AppError, Result},
    model::FileChange,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathMapping {
    pub from: String,
    pub to: String,
}

pub struct Mapper {
    mappings: Vec<PathMapping>,
}

impl Mapper {
    pub fn new(mut mappings: Vec<PathMapping>) -> Self {
        for m in &mut mappings {
            m.from = normalize_mapping_prefix(&m.from);
        }
        // Longest prefix first so `/trunk/foo` wins over `/trunk` and `/`.
        mappings.sort_by(|a, b| b.from.len().cmp(&a.from.len()));
        Self { mappings }
    }

    /// Apply the first matching mapping to `path`.
    /// Returns an error if no mapping covers the path, to prevent silent misrouting.
    pub fn map(&self, path: &str) -> Result<String> {
        let path = normalize_source_path(path);
        for m in &self.mappings {
            if let Some(suffix) = strip_mapped_prefix(&path, &m.from) {
                return Ok(join_target(&m.to, suffix));
            }
        }
        Err(AppError::Mapping(format!(
            "No mapping found for path '{path}'. \
             SVN paths are often like '/src/...' when the repo URL already points at trunk — \
             try adding a rule: / → . (or match your diff path prefix)."
        )))
    }

    /// Apply mapping to every FileChange in place.
    pub fn apply_to_files(&self, files: &mut Vec<FileChange>) -> Result<()> {
        for fc in files.iter_mut() {
            fc.target_path = Some(self.map(&fc.path)?);
            if let Some(old_path) = fc.old_path.as_deref() {
                fc.old_path = Some(self.map(old_path)?);
            }
        }
        Ok(())
    }
}

fn normalize_mapping_prefix(from: &str) -> String {
    let t = from.trim();
    if t.is_empty() || t == "." {
        "/".to_string()
    } else if t.starts_with('/') {
        t.to_string()
    } else {
        format!("/{t}")
    }
}

fn normalize_source_path(path: &str) -> String {
    let p = path.trim().replace('\\', "/");
    if p.starts_with('/') {
        p
    } else {
        format!("/{p}")
    }
}

/// Strip `prefix` only on a path segment boundary (`/trunk` matches `/trunk/x` but not `/trunk_backup`).
fn strip_mapped_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix == "/" {
        return Some(path.trim_start_matches('/'));
    }
    let rest = path.strip_prefix(prefix)?;
    if rest.is_empty() || rest.starts_with('/') {
        Some(rest.trim_start_matches('/'))
    } else {
        None
    }
}

fn join_target(to: &str, suffix: &str) -> String {
    let base = to.trim().trim_end_matches('/');
    if suffix.is_empty() {
        if base.is_empty() || base == "." {
            ".".to_string()
        } else {
            base.to_string()
        }
    } else if base.is_empty() || base == "." {
        suffix.to_string()
    } else {
        format!("{base}/{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mapper() -> Mapper {
        Mapper::new(vec![
            PathMapping {
                from: "/trunk/module-a".into(),
                to: "packages/module-a".into(),
            },
            PathMapping {
                from: "/trunk".into(),
                to: ".".into(),
            },
        ])
    }

    #[test]
    fn maps_known_prefix() {
        let m = make_mapper();
        assert_eq!(
            m.map("/trunk/module-a/src/foo.rs").unwrap(),
            "packages/module-a/src/foo.rs"
        );
    }

    #[test]
    fn maps_root_prefix() {
        let m = make_mapper();
        assert_eq!(m.map("/trunk/other.rs").unwrap(), "other.rs");
    }

    #[test]
    fn maps_repo_relative_paths_with_root_rule() {
        let m = Mapper::new(vec![
            PathMapping {
                from: "/trunk".into(),
                to: ".".into(),
            },
            PathMapping {
                from: "/".into(),
                to: ".".into(),
            },
        ]);
        assert_eq!(m.map("/src/main.rs").unwrap(), "src/main.rs");
        assert_eq!(m.map("/README.md").unwrap(), "README.md");
    }

    #[test]
    fn dot_source_prefix_maps_repo_root() {
        let m = Mapper::new(vec![PathMapping {
            from: ".".into(),
            to: ".".into(),
        }]);
        assert_eq!(m.map("/src/main.rs").unwrap(), "src/main.rs");
    }

    #[test]
    fn trunk_prefix_does_not_match_trunk_backup() {
        let m = Mapper::new(vec![PathMapping {
            from: "/trunk".into(),
            to: ".".into(),
        }]);
        assert!(m.map("/trunk_backup/x.rs").is_err());
    }

    #[test]
    fn returns_error_for_unmapped_path() {
        let m = make_mapper();
        assert!(m.map("/branches/experiment/foo.rs").is_err());
    }

    #[test]
    fn maps_exact_prefix_match() {
        let m = make_mapper();
        assert_eq!(m.map("/trunk/module-a").unwrap(), "packages/module-a");
    }
}
