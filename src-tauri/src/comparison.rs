use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use encoding_rs::GBK;
use sha2::{Digest, Sha256};

use crate::{
    diff::line_diff::{diff_with_context, lines_to_diff, DEFAULT_DIFF_CONTEXT_LINES},
    model::{FileChange, FileChangeKind},
    store::models::{
        FileComparisonStatus, FileComparisonSummary, FileContentKind, FileStatus,
        MigrationComparisonResult, MigrationFileComparison, MigrationFilePair, MigrationRecord,
    },
};

pub fn migration_file_pairs(files: &[FileChange]) -> Vec<MigrationFilePair> {
    files
        .iter()
        .map(|file| {
            let target_path = file
                .target_path
                .clone()
                .unwrap_or_else(|| file.path.clone());
            MigrationFilePair {
                id: target_path.clone(),
                source_path: file.path.clone(),
                target_path,
                status: file_status(&file.kind),
                is_binary: file.kind == FileChangeKind::Binary,
            }
        })
        .collect()
}

pub fn compare_record_files(
    record: &MigrationRecord,
    pairs: &[MigrationFilePair],
) -> MigrationComparisonResult {
    let files: Vec<MigrationFileComparison> = pairs
        .iter()
        .map(|pair| compare_file_pair(record, pair))
        .collect();
    let summary = comparison_summary(&files);

    MigrationComparisonResult {
        migration_id: record.id.clone(),
        compared_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        available: true,
        reason: None,
        summary,
        files,
    }
}

pub fn unavailable_comparison(
    record: &MigrationRecord,
    reason: String,
) -> MigrationComparisonResult {
    MigrationComparisonResult {
        migration_id: record.id.clone(),
        compared_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        available: false,
        reason: Some(reason),
        summary: FileComparisonSummary::default(),
        files: Vec::new(),
    }
}

/// Old history records did not retain source/target path pairs. Recover only
/// when the original source paths exactly match every stored target path.
pub fn legacy_identity_pairs(
    record: &MigrationRecord,
    source_files: &[FileChange],
) -> std::result::Result<Vec<MigrationFilePair>, String> {
    if source_files.len() != record.files.len() {
        return Err("旧记录缺少可靠路径映射，无法安全对比".into());
    }

    let mut source_by_key = BTreeMap::new();
    for source in source_files {
        let key = path_key(&source.path)?;
        if source_by_key.insert(key, source).is_some() {
            return Err("旧记录存在重复源路径，无法安全对比".into());
        }
    }

    let mut pairs = Vec::with_capacity(record.files.len());
    for target in &record.files {
        let key = path_key(&target.path)?;
        let Some(source) = source_by_key.remove(&key) else {
            return Err("旧记录缺少可靠路径映射，无法安全对比".into());
        };
        pairs.push(MigrationFilePair {
            id: target.id.clone(),
            source_path: source.path.clone(),
            target_path: target.path.clone(),
            status: target.status.clone(),
            is_binary: source.kind == FileChangeKind::Binary,
        });
    }

    if source_by_key.is_empty() {
        Ok(pairs)
    } else {
        Err("旧记录缺少可靠路径映射，无法安全对比".into())
    }
}

fn compare_file_pair(
    record: &MigrationRecord,
    pair: &MigrationFilePair,
) -> MigrationFileComparison {
    let source = read_side(&record.source.path, &pair.source_path, "源");
    let target = read_side(&record.target.path, &pair.target_path, "目标");

    match (source, target) {
        (Err(source_err), Err(target_err)) => {
            unreadable(pair, format!("{source_err}；{target_err}"))
        }
        (Err(err), _) | (_, Err(err)) => unreadable(pair, err),
        (Ok(FileRead::Missing), Ok(FileRead::Missing)) => MigrationFileComparison {
            id: pair.id.clone(),
            source_path: pair.source_path.clone(),
            target_path: pair.target_path.clone(),
            status: FileComparisonStatus::BothMissing,
            content_kind: None,
            source_size: None,
            target_size: None,
            source_sha256: None,
            target_sha256: None,
            diff: None,
            message: Some("两侧文件均不存在".into()),
        },
        (Ok(FileRead::Missing), Ok(FileRead::Present(target))) => missing_result(
            pair,
            FileComparisonStatus::SourceMissing,
            None,
            Some(target),
        ),
        (Ok(FileRead::Present(source)), Ok(FileRead::Missing)) => missing_result(
            pair,
            FileComparisonStatus::TargetMissing,
            Some(source),
            None,
        ),
        (Ok(FileRead::Present(source)), Ok(FileRead::Present(target))) => {
            present_result(pair, source, target)
        }
    }
}

fn present_result(
    pair: &MigrationFilePair,
    source: Vec<u8>,
    target: Vec<u8>,
) -> MigrationFileComparison {
    let source_sha256 = sha256(&source);
    let target_sha256 = sha256(&target);
    let source_size = source.len() as u64;
    let target_size = target.len() as u64;
    let source_text = (!pair.is_binary).then(|| decode_text(&source)).flatten();
    let target_text = (!pair.is_binary).then(|| decode_text(&target)).flatten();
    let content_kind = if source_text.is_some() && target_text.is_some() {
        FileContentKind::Text
    } else {
        FileContentKind::Binary
    };

    if source == target {
        return MigrationFileComparison {
            id: pair.id.clone(),
            source_path: pair.source_path.clone(),
            target_path: pair.target_path.clone(),
            status: FileComparisonStatus::Identical,
            content_kind: Some(content_kind),
            source_size: Some(source_size),
            target_size: Some(target_size),
            source_sha256: Some(source_sha256),
            target_sha256: Some(target_sha256),
            diff: None,
            message: None,
        };
    }

    if let (Some(source_text), Some(target_text)) = (source_text, target_text) {
        let source_normalized = normalize_text(&source_text);
        let target_normalized = normalize_text(&target_text);
        let format_only = source_normalized == target_normalized;
        return MigrationFileComparison {
            id: pair.id.clone(),
            source_path: pair.source_path.clone(),
            target_path: pair.target_path.clone(),
            status: if format_only {
                FileComparisonStatus::FormatOnly
            } else {
                FileComparisonStatus::Different
            },
            content_kind: Some(FileContentKind::Text),
            source_size: Some(source_size),
            target_size: Some(target_size),
            source_sha256: Some(source_sha256),
            target_sha256: Some(target_sha256),
            diff: (!format_only).then(|| {
                diff_with_context(
                    lines_to_diff(Some(&source_normalized), Some(&target_normalized)),
                    DEFAULT_DIFF_CONTEXT_LINES,
                )
            }),
            message: format_only.then(|| "原始字节不同，但文本内容一致".into()),
        };
    }

    MigrationFileComparison {
        id: pair.id.clone(),
        source_path: pair.source_path.clone(),
        target_path: pair.target_path.clone(),
        status: FileComparisonStatus::Different,
        content_kind: Some(FileContentKind::Binary),
        source_size: Some(source_size),
        target_size: Some(target_size),
        source_sha256: Some(source_sha256),
        target_sha256: Some(target_sha256),
        diff: None,
        message: Some("二进制文件的原始字节不同".into()),
    }
}

fn missing_result(
    pair: &MigrationFilePair,
    status: FileComparisonStatus,
    source: Option<Vec<u8>>,
    target: Option<Vec<u8>>,
) -> MigrationFileComparison {
    let source_meta = source
        .as_deref()
        .map(|bytes| file_meta(bytes, pair.is_binary));
    let target_meta = target
        .as_deref()
        .map(|bytes| file_meta(bytes, pair.is_binary));
    let content_kind = source_meta
        .as_ref()
        .or(target_meta.as_ref())
        .map(|meta| meta.0);
    MigrationFileComparison {
        id: pair.id.clone(),
        source_path: pair.source_path.clone(),
        target_path: pair.target_path.clone(),
        status,
        content_kind,
        source_size: source_meta.as_ref().map(|meta| meta.1),
        target_size: target_meta.as_ref().map(|meta| meta.1),
        source_sha256: source_meta.map(|meta| meta.2),
        target_sha256: target_meta.map(|meta| meta.2),
        diff: None,
        message: Some(match status {
            FileComparisonStatus::SourceMissing => "源仓库中不存在该文件".into(),
            FileComparisonStatus::TargetMissing => "目标仓库中不存在该文件".into(),
            _ => String::new(),
        }),
    }
}

fn unreadable(pair: &MigrationFilePair, message: String) -> MigrationFileComparison {
    MigrationFileComparison {
        id: pair.id.clone(),
        source_path: pair.source_path.clone(),
        target_path: pair.target_path.clone(),
        status: FileComparisonStatus::Unreadable,
        content_kind: None,
        source_size: None,
        target_size: None,
        source_sha256: None,
        target_sha256: None,
        diff: None,
        message: Some(message),
    }
}

fn comparison_summary(files: &[MigrationFileComparison]) -> FileComparisonSummary {
    let mut summary = FileComparisonSummary {
        total: files.len() as u32,
        ..FileComparisonSummary::default()
    };
    for file in files {
        match file.status {
            FileComparisonStatus::Identical | FileComparisonStatus::BothMissing => {
                summary.identical += 1;
            }
            FileComparisonStatus::FormatOnly => summary.format_only += 1,
            FileComparisonStatus::Different => summary.different += 1,
            FileComparisonStatus::SourceMissing | FileComparisonStatus::TargetMissing => {
                summary.missing += 1;
            }
            FileComparisonStatus::Unreadable => summary.unreadable += 1,
        }
    }
    summary
}

fn file_status(kind: &FileChangeKind) -> FileStatus {
    match kind {
        FileChangeKind::Add => FileStatus::Add,
        FileChangeKind::Delete => FileStatus::Del,
        _ => FileStatus::Mod,
    }
}

enum FileRead {
    Missing,
    Present(Vec<u8>),
}

fn read_side(root: &str, path: &str, label: &str) -> std::result::Result<FileRead, String> {
    let resolved =
        resolve_under_root(root, path).map_err(|err| format!("{label}文件路径无效：{err}"))?;
    match fs::read(&resolved) {
        Ok(bytes) => Ok(FileRead::Present(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(FileRead::Missing),
        Err(err) => Err(format!("读取{label}文件失败：{err}")),
    }
}

fn resolve_under_root(root: &str, logical_path: &str) -> std::result::Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|err| format!("仓库目录不可访问：{err}"))?;
    let relative = relative_path(logical_path)?;
    let candidate = root.join(relative);

    let mut ancestor = candidate.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| "文件路径不在仓库目录内".to_string())?;
    }
    let canonical_ancestor =
        fs::canonicalize(ancestor).map_err(|err| format!("无法验证文件路径：{err}"))?;
    if !canonical_ancestor.starts_with(&root) {
        return Err("文件路径不在仓库目录内".into());
    }
    Ok(candidate)
}

fn path_key(path: &str) -> std::result::Result<String, String> {
    Ok(relative_path(path)?.to_string_lossy().replace('\\', "/"))
}

fn relative_path(logical_path: &str) -> std::result::Result<PathBuf, String> {
    let normalized = logical_path.trim().replace('\\', "/");
    let without_root = normalized.trim_start_matches('/');
    let without_dot = without_root.strip_prefix("./").unwrap_or(without_root);
    if without_dot.is_empty() || without_dot == "." {
        return Err("文件路径为空".into());
    }

    let path = Path::new(without_dot);
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => cleaned.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("不允许绝对路径或上级目录".into());
            }
        }
    }
    if cleaned.as_os_str().is_empty() {
        return Err("文件路径为空".into());
    }
    Ok(cleaned)
}

fn decode_text(bytes: &[u8]) -> Option<String> {
    if bytes.contains(&0) {
        return None;
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Some(text.to_string());
    }
    let (decoded, _, had_errors) = GBK.decode(bytes);
    (!had_errors).then(|| decoded.into_owned())
}

fn normalize_text(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .strip_suffix('\n')
        .unwrap_or(&normalized)
        .to_string()
}

fn file_meta(bytes: &[u8], force_binary: bool) -> (FileContentKind, u64, String) {
    let kind = if !force_binary && decode_text(bytes).is_some() {
        FileContentKind::Text
    } else {
        FileContentKind::Binary
    };
    (kind, bytes.len() as u64, sha256(bytes))
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::store::models::{CommitSnapshot, DiffLine, FileChangeView, RepoSnapshot, RepoType};

    struct TempRoots {
        base: PathBuf,
        source: PathBuf,
        target: PathBuf,
    }

    impl TempRoots {
        fn new() -> Self {
            let base =
                std::env::temp_dir().join(format!("copy-diff-compare-{}", uuid::Uuid::new_v4()));
            let source = base.join("source");
            let target = base.join("target");
            fs::create_dir_all(&source).unwrap();
            fs::create_dir_all(&target).unwrap();
            Self {
                base,
                source,
                target,
            }
        }

        fn write_source(&self, name: &str, content: &[u8]) {
            let path = self.source.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }

        fn write_target(&self, name: &str, content: &[u8]) {
            let path = self.target.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }

        fn record(&self, pairs: Vec<MigrationFilePair>) -> MigrationRecord {
            MigrationRecord {
                id: "m1".into(),
                completed_at: "2026-08-10 12:00".into(),
                source: RepoSnapshot {
                    name: "source".into(),
                    path: self.source.to_string_lossy().into_owned(),
                    repo_type: RepoType::Git,
                    branch: "main".into(),
                },
                target: RepoSnapshot {
                    name: "target".into(),
                    path: self.target.to_string_lossy().into_owned(),
                    repo_type: RepoType::Git,
                    branch: "main".into(),
                },
                commits: Vec::new(),
                files: Vec::new(),
                comparison_files: pairs,
                conflicts_resolved: 0,
                status: "success".into(),
            }
        }
    }

    impl Drop for TempRoots {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn pair(path: &str) -> MigrationFilePair {
        MigrationFilePair {
            id: path.into(),
            source_path: path.into(),
            target_path: path.into(),
            status: FileStatus::Mod,
            is_binary: false,
        }
    }

    #[test]
    fn stores_source_and_target_paths_for_mapped_migrations() {
        let file = FileChange {
            path: "/trunk/module-a/src/lib.rs".into(),
            target_path: Some("packages/module-a/src/lib.rs".into()),
            kind: FileChangeKind::Modify,
            old_path: None,
            before: None,
            after: None,
            source_after: None,
            after_bytes: None,
            source_ref: None,
            patch: None,
            conflict_risk: None,
            analysis: None,
        };

        let pairs = migration_file_pairs(&[file]);

        assert_eq!(pairs[0].source_path, "/trunk/module-a/src/lib.rs");
        assert_eq!(pairs[0].target_path, "packages/module-a/src/lib.rs");
    }

    #[test]
    fn compares_equal_text_and_format_only_text() {
        let roots = TempRoots::new();
        roots.write_source("same.txt", b"same\n");
        roots.write_target("same.txt", b"same\n");
        roots.write_source("format.txt", b"\xEF\xBB\xBFone\r\ntwo\r\n");
        roots.write_target("format.txt", b"one\ntwo");
        let record = roots.record(vec![pair("same.txt"), pair("format.txt")]);

        let result = compare_record_files(&record, &record.comparison_files);

        assert_eq!(result.summary.identical, 1);
        assert_eq!(result.summary.format_only, 1);
        assert!(result.files[1].diff.is_none());
    }

    #[test]
    fn compares_text_binary_and_missing_files() {
        let roots = TempRoots::new();
        roots.write_source("text.txt", b"old\n");
        roots.write_target("text.txt", b"new\n");
        roots.write_source("binary.bin", &[0, 1, 2]);
        roots.write_target("binary.bin", &[0, 1, 3]);
        roots.write_target("target-only.txt", b"target");
        let mut binary = pair("binary.bin");
        binary.is_binary = true;
        let record = roots.record(vec![
            pair("text.txt"),
            binary,
            pair("target-only.txt"),
            pair("gone.txt"),
        ]);

        let result = compare_record_files(&record, &record.comparison_files);

        assert_eq!(result.summary.different, 2);
        assert_eq!(result.summary.missing, 1);
        assert_eq!(result.summary.identical, 1);
        assert!(result.files[0]
            .diff
            .as_ref()
            .is_some_and(|diff| !diff.is_empty()));
        assert_eq!(result.files[1].content_kind, Some(FileContentKind::Binary));
        assert_eq!(result.files[3].status, FileComparisonStatus::BothMissing);
    }

    #[test]
    fn rejects_path_outside_repository() {
        let roots = TempRoots::new();
        let record = roots.record(vec![pair("../outside.txt")]);

        let result = compare_record_files(&record, &record.comparison_files);

        assert_eq!(result.files[0].status, FileComparisonStatus::Unreadable);
    }

    #[test]
    fn old_records_default_comparison_files_and_recover_identity_pairs() {
        let roots = TempRoots::new();
        let mut record = roots.record(Vec::new());
        record.files = vec![FileChangeView {
            id: "src/a.txt".into(),
            path: "src/a.txt".into(),
            status: FileStatus::Mod,
            additions: 1,
            deletions: 1,
            diff: Some(Vec::<DiffLine>::new()),
        }];
        record.commits = vec![CommitSnapshot {
            id: "git:abc".into(),
            hash: "abc".into(),
            msg: "change".into(),
            author: "author".into(),
            date: "2026-08-10T00:00:00Z".into(),
            files: 1,
        }];
        let mut json = serde_json::to_value(&record).unwrap();
        json.as_object_mut().unwrap().remove("comparison_files");
        let legacy: MigrationRecord = serde_json::from_value(json).unwrap();
        let source = FileChange {
            path: "src/a.txt".into(),
            target_path: None,
            kind: FileChangeKind::Modify,
            old_path: None,
            before: None,
            after: None,
            source_after: None,
            after_bytes: None,
            source_ref: None,
            patch: None,
            conflict_risk: None,
            analysis: None,
        };

        let pairs = legacy_identity_pairs(&legacy, &[source]).unwrap();

        assert_eq!(legacy.comparison_files.len(), 0);
        assert_eq!(pairs[0].source_path, "src/a.txt");
    }

    #[test]
    fn old_records_refuse_non_identity_paths() {
        let roots = TempRoots::new();
        let mut record = roots.record(Vec::new());
        record.files = vec![FileChangeView {
            id: "target/a.txt".into(),
            path: "target/a.txt".into(),
            status: FileStatus::Mod,
            additions: 0,
            deletions: 0,
            diff: None,
        }];
        let source = FileChange {
            path: "source/a.txt".into(),
            target_path: None,
            kind: FileChangeKind::Modify,
            old_path: None,
            before: None,
            after: None,
            source_after: None,
            after_bytes: None,
            source_ref: None,
            patch: None,
            conflict_risk: None,
            analysis: None,
        };

        assert!(legacy_identity_pairs(&record, &[source]).is_err());
    }
}
