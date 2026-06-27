use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize)]
pub struct SvnWcInfo {
    pub branch: String,
    pub relative_url: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct InfoRoot {
    entry: InfoEntry,
}

#[derive(Debug, Deserialize)]
struct InfoEntry {
    url: String,
    #[serde(rename = "relative-url")]
    relative_url: String,
}

/// Last path segment of SVN `relative-url` (e.g. `^/.../hk` → `hk`).
pub fn branch_from_relative_url(relative_url: &str) -> Option<String> {
    let path = relative_url.strip_prefix('^')?.trim_start_matches('/');
    if path.trim().is_empty() {
        return Some("trunk".into());
    }
    let segment = path.rsplit('/').next()?.trim();
    if segment.is_empty() {
        None
    } else {
        Some(segment.to_string())
    }
}

pub fn parse_info_xml(xml: &str) -> Result<SvnWcInfo> {
    let root: InfoRoot =
        from_str(xml).map_err(|e| AppError::Vcs(format!("failed to parse svn info XML: {e}")))?;
    let branch = branch_from_relative_url(&root.entry.relative_url)
        .ok_or_else(|| AppError::Vcs("cannot detect checkout directory from svn info".into()))?;
    Ok(SvnWcInfo {
        branch,
        relative_url: root.entry.relative_url,
        url: root.entry.url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_branch_from_relative_url() {
        assert_eq!(
            branch_from_relative_url("^/01IT项目/01开发项目/18IMS系统/hk").as_deref(),
            Some("hk")
        );
        assert_eq!(
            branch_from_relative_url("^/trunk").as_deref(),
            Some("trunk")
        );
        assert_eq!(branch_from_relative_url("^/").as_deref(), Some("trunk"));
    }
}
