use serde::{Deserialize, Serialize};

use crate::mapper::PathMapping;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCheckState {
    pub last_checked_at: Option<String>,
    pub available_version: Option<String>,
    pub available_notes: Option<String>,
    pub available_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoType {
    Git,
    Svn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub repo_type: RepoType,
    pub branch: String,
    pub last_used: Option<String>,
    pub svn_user: Option<String>,
    /// Encrypted at rest; never returned to frontend on list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svn_pass_encrypted: Option<String>,
    pub path_mappings: Vec<PathMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInput {
    pub id: Option<String>,
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub repo_type: RepoType,
    pub branch: String,
    pub svn_user: Option<String>,
    /// Plain password from UI; empty means keep existing when editing.
    pub svn_pass: Option<String>,
    pub path_mappings: Vec<PathMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoView {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub repo_type: RepoType,
    pub branch: String,
    pub last_used: Option<String>,
    pub svn_user: Option<String>,
    pub has_svn_pass: bool,
    pub path_mappings: Vec<PathMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorRecord {
    pub id: String,
    pub name: String,
    pub exe: String,
    pub kind: String,
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSnapshot {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub repo_type: RepoType,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSnapshot {
    pub id: String,
    pub hash: String,
    pub msg: String,
    pub author: String,
    pub date: String,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Add,
    Mod,
    Del,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineType {
    Ctx,
    Add,
    Del,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    #[serde(rename = "type")]
    pub line_type: DiffLineType,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeView {
    pub id: String,
    pub path: String,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<Vec<DiffLine>>,
}

/// Source/target paths for one file that participated in a completed migration.
/// Stored with the history record so later comparisons do not need to infer a
/// custom mapping from current repository settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationFilePair {
    pub id: String,
    pub source_path: String,
    pub target_path: String,
    pub status: FileStatus,
    #[serde(default)]
    pub is_binary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileComparisonStatus {
    Identical,
    FormatOnly,
    Different,
    SourceMissing,
    TargetMissing,
    BothMissing,
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileContentKind {
    Text,
    Binary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileComparisonSummary {
    pub total: u32,
    pub identical: u32,
    pub format_only: u32,
    pub different: u32,
    pub missing: u32,
    pub unreadable: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationFileComparison {
    pub id: String,
    pub source_path: String,
    pub target_path: String,
    pub status: FileComparisonStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_kind: Option<FileContentKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<Vec<DiffLine>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationComparisonResult {
    pub migration_id: String,
    pub compared_at: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub summary: FileComparisonSummary,
    pub files: Vec<MigrationFileComparison>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationMode {
    IncrementalFirst,
    CommitResult,
    StrictReplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStatus {
    AutoOk,
    Review,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStrategy {
    Skip,
    ApplyPatch,
    WriteAfter,
    ManualMerge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationItemView {
    pub id: String,
    pub path: String,
    pub status: FileStatus,
    pub integration_status: IntegrationStatus,
    pub strategy: IntegrationStrategy,
    pub reason: String,
    pub overlap_lines: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_status: Option<crate::model::LocationStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_status: Option<crate::model::MergeStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_method: Option<crate::model::MatchMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<usize>,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub diff: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationPlanResult {
    pub mode: MigrationMode,
    pub auto_ok_count: u32,
    pub review_count: u32,
    pub blocked_count: u32,
    pub items: Vec<IntegrationItemView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictView {
    pub id: String,
    pub path: String,
    pub status: FileStatus,
    pub reason: String,
    pub overlap_lines: Option<[u32; 2]>,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub id: String,
    pub completed_at: String,
    pub source: RepoSnapshot,
    pub target: RepoSnapshot,
    pub commits: Vec<CommitSnapshot>,
    pub files: Vec<FileChangeView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comparison_files: Vec<MigrationFilePair>,
    pub conflicts_resolved: u32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoPairMappingRecord {
    pub source_id: String,
    pub target_id: String,
    pub path_mappings: Vec<PathMapping>,
    pub custom_mapping: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoPairMappingInput {
    pub source_id: String,
    pub target_id: String,
    pub path_mappings: Vec<PathMapping>,
    pub custom_mapping: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoPairMappingView {
    pub path_mappings: Vec<PathMapping>,
    pub custom_mapping: bool,
}

pub fn default_svn_mappings(branch: &str) -> Vec<PathMapping> {
    let trimmed = branch.trim().trim_matches('/');
    let from_branch = if trimmed.is_empty() {
        "/trunk".to_string()
    } else {
        format!("/{trimmed}")
    };
    vec![
        PathMapping {
            from: from_branch,
            to: ".".into(),
        },
        PathMapping {
            from: "/".into(),
            to: ".".into(),
        },
    ]
}

pub const EDITOR_PRESETS: &[(&str, &str, &str, &str)] = &[
    ("vscode", "VS Code", "vscode", "Code.exe"),
    ("vs", "Visual Studio", "visualstudio", "devenv.exe"),
    ("cursor", "Cursor", "cursor", "Cursor.exe"),
    ("explorer", "File Explorer", "explorer", "explorer.exe"),
    ("terminal", "Terminal", "terminal", "wt.exe"),
    ("gitbash", "Git Bash", "gitbash", "git-bash.exe"),
    ("idea", "IntelliJ IDEA", "idea", "idea64.exe"),
    ("pycharm", "PyCharm", "pycharm", "pycharm64.exe"),
];
