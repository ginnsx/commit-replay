use quick_xml::de::from_str;
use serde::Deserialize;

use crate::{
    error::{AppError, Result},
    model::FileChangeKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvnDiffSummaryEntry {
    pub url: String,
    pub path: String,
    pub kind: FileChangeKind,
}

#[derive(Debug, Deserialize)]
struct DiffRoot {
    paths: DiffPaths,
}

#[derive(Debug, Deserialize)]
struct DiffPaths {
    #[serde(rename = "path", default)]
    path: Vec<RawPathEntry>,
}

#[derive(Debug, Deserialize)]
struct RawPathEntry {
    #[serde(rename = "@item")]
    item: String,
    #[serde(rename = "@kind")]
    node_kind: String,
    #[serde(rename = "$text")]
    url: String,
}

/// 解析 `svn diff --summarize --xml`，从 UTF-8 URL 恢复不受控制台编码影响的路径。
pub fn parse_diff_summary_xml(xml: &str, branch_url: &str) -> Result<Vec<SvnDiffSummaryEntry>> {
    let root: DiffRoot = from_str(xml)
        .map_err(|e| AppError::Vcs(format!("failed to parse svn diff summary XML: {e}")))?;
    let base = branch_url.trim_end_matches('/');
    let mut entries = Vec::new();

    for raw in root.paths.path {
        if raw.node_kind != "file" {
            continue;
        }
        let Some(encoded_path) = raw.url.strip_prefix(base) else {
            return Err(AppError::Vcs(format!(
                "svn diff path is outside branch URL: {}",
                raw.url
            )));
        };
        if !encoded_path.starts_with('/') {
            return Err(AppError::Vcs(format!(
                "svn diff path is outside branch URL: {}",
                raw.url
            )));
        }
        let decoded = percent_decode(encoded_path.trim_start_matches('/'))?;
        let kind = match raw.item.as_str() {
            "added" => FileChangeKind::Add,
            "deleted" => FileChangeKind::Delete,
            "modified" | "replaced" => FileChangeKind::Modify,
            // 只有属性变化时没有可迁移的文件内容。
            "none" => continue,
            other => {
                return Err(AppError::Vcs(format!(
                    "unsupported svn diff summary item: {other}"
                )))
            }
        };
        entries.push(SvnDiffSummaryEntry {
            url: raw.url,
            path: format!("/{decoded}"),
            kind,
        });
    }

    Ok(entries)
}

/// 把工作副本相对路径拼成只含 ASCII 的 SVN URL，避免 Windows 客户端损坏命令行参数。
pub fn encoded_child_url(branch_url: &str, path: &str) -> String {
    let encoded = path
        .trim_start_matches('/')
        .split('/')
        .map(percent_encode_segment)
        .collect::<Vec<_>>()
        .join("/");
    format!("{}/{}", branch_url.trim_end_matches('/'), encoded)
}

fn percent_encode_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn percent_decode(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(AppError::Vcs("invalid percent-encoded svn URL".into()));
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded)
        .map_err(|e| AppError::Vcs(format!("svn URL path is not valid UTF-8: {e}")))
}

fn hex_value(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(AppError::Vcs("invalid percent-encoded svn URL".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_percent_encoded_chinese_path() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<diff><paths><path props="none" kind="file" item="added">file:///repo/trunk/src/%E4%B8%AD%E6%96%87%20%E8%B7%AF%E5%BE%84.txt</path></paths></diff>"#;
        let entries = parse_diff_summary_xml(xml, "file:///repo/trunk").unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "/src/中文 路径.txt");
        assert_eq!(entries[0].kind, FileChangeKind::Add);
    }

    #[test]
    fn skips_directory_and_property_only_entries() {
        let xml = r#"<diff><paths>
<path props="none" kind="dir" item="added">https://example.test/svn/trunk/src</path>
<path props="modified" kind="file" item="none">https://example.test/svn/trunk/a.txt</path>
</paths></diff>"#;

        assert!(
            parse_diff_summary_xml(xml, "https://example.test/svn/trunk")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn encodes_each_url_path_segment() {
        assert_eq!(
            encoded_child_url("https://example.test/svn/trunk/", "/src/中文 路径.txt"),
            "https://example.test/svn/trunk/src/%E4%B8%AD%E6%96%87%20%E8%B7%AF%E5%BE%84.txt"
        );
    }
}
