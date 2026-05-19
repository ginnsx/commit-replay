use crate::{error::{AppError, Result}, model::FileChange};

#[derive(Debug, Clone)]
pub struct PathMapping {
    pub from: String,
    pub to: String,
}

pub struct Mapper {
    mappings: Vec<PathMapping>,
}

impl Mapper {
    pub fn new(mappings: Vec<PathMapping>) -> Self {
        Self { mappings }
    }

    /// Apply the first matching mapping to `path`.
    /// Returns an error if no mapping covers the path, to prevent silent misrouting.
    pub fn map(&self, path: &str) -> Result<String> {
        for m in &self.mappings {
            if let Some(suffix) = path.strip_prefix(m.from.as_str()) {
                let suffix = suffix.trim_start_matches('/');
                return Ok(if suffix.is_empty() {
                    m.to.clone()
                } else {
                    format!("{}/{}", m.to.trim_end_matches('/'), suffix)
                });
            }
        }
        Err(AppError::Mapping(format!(
            "No mapping found for path '{}'. Add an entry in config mappings.",
            path
        )))
    }

    /// Apply mapping to every FileChange in place.
    pub fn apply_to_files(&self, files: &mut Vec<FileChange>) -> Result<()> {
        for fc in files.iter_mut() {
            fc.target_path = Some(self.map(&fc.path)?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mapper() -> Mapper {
        Mapper::new(vec![
            PathMapping { from: "/trunk/module-a".into(), to: "packages/module-a".into() },
            PathMapping { from: "/trunk".into(), to: ".".into() },
        ])
    }

    #[test]
    fn maps_known_prefix() {
        let m = make_mapper();
        assert_eq!(m.map("/trunk/module-a/src/foo.rs").unwrap(), "packages/module-a/src/foo.rs");
    }

    #[test]
    fn maps_root_prefix() {
        let m = make_mapper();
        assert_eq!(m.map("/trunk/other.rs").unwrap(), "./other.rs");
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
