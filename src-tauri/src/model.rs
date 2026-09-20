use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// VCS identity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VcsKind {
    Svn,
    Git,
}

// ---------------------------------------------------------------------------
// Commit list (one row in the UI list)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayUnitMeta {
    /// "svn:12345" or "git:abc1234"
    pub source_ref: String,
    pub author: String,
    pub date: String,
    pub message: String,
    pub changed_paths_count: usize,
}

// ---------------------------------------------------------------------------
// File-level change (VCS-agnostic)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileChangeKind {
    Add,
    Modify,
    Delete,
    Rename,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictRisk {
    Low,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationStatus {
    Exact,
    ContextDrift,
    MultipleCandidates,
    NotFound,
    AlreadyContains,
    SameRegionConflict,
    FileMissing,
    PathUnmapped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    OldBlock,
    Context,
    Fuzzy,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStatus {
    AutoApply,
    KeepTarget,
    AutoMerge,
    Conflict,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeAnalysis {
    pub target_path: Option<String>,
    pub location_status: LocationStatus,
    pub match_method: MatchMethod,
    pub confidence: f32,
    pub candidate_count: usize,
    pub merge_status: MergeStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Source-side path (before mapping)
    pub path: String,
    /// Target-side path (after mapping); None until mapper runs
    pub target_path: Option<String>,
    pub kind: FileChangeKind,
    /// Populated for rename: original source path
    pub old_path: Option<String>,
    /// Current content in target working copy (None = file does not exist yet)
    pub before: Option<String>,
    /// Expected content after apply
    pub after: Option<String>,
    /// Post-change content on the source side (e.g. svn cat @ revision)
    pub source_after: Option<String>,
    /// Raw post-change content for binary files. Kept backend-only.
    #[serde(skip)]
    pub after_bytes: Option<Vec<u8>>,
    /// Source commit ref for lazy source_after loading
    pub source_ref: Option<String>,
    /// Raw unified diff hunk (text files only)
    pub patch: Option<String>,
    pub conflict_risk: Option<ConflictRisk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<ChangeAnalysis>,
}

// ---------------------------------------------------------------------------
// ChangeSet: one source commit's worth of changes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSet {
    pub meta: ReplayUnitMeta,
    pub files: Vec<FileChange>,
}

// ---------------------------------------------------------------------------
// ReplayPlan: the full ordered list of units to apply
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayPolicy {
    /// Stop the batch on first failure (default)
    FailStop,
    /// Skip failed units, continue the rest
    SkipFailed,
    /// Pause and wait for manual resolution on each failure
    ManualOnFail,
}

impl Default for ReplayPolicy {
    fn default() -> Self {
        Self::FailStop
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayPlan {
    pub units: Vec<ChangeSet>,
    pub policy: ReplayPolicy,
}

// ---------------------------------------------------------------------------
// Preview result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub binary_count: usize,
    pub conflict_risk_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewUnit {
    pub meta: ReplayUnitMeta,
    pub files: Vec<FileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewResult {
    pub units: Vec<PreviewUnit>,
    /// All files merged by target_path across all units
    pub aggregated: Vec<FileChange>,
    pub stats: DiffStats,
}

// ---------------------------------------------------------------------------
// Apply / validation results
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyStatus {
    Ok,
    Conflict,
    MissingBase,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub status: ApplyStatus,
    pub message: Option<String>,
    /// Paths that failed to apply cleanly
    pub failed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub passed: bool,
    pub issues: Vec<ValidationIssue>,
}

// ---------------------------------------------------------------------------
// Unit execution state (sent as Tauri events to the frontend)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitStatus {
    Pending,
    Applying,
    Committed,
    Failed,
    Skipped,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitStatusEvent {
    pub source_ref: String,
    pub status: UnitStatus,
    pub target_ref: Option<String>,
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// Audit log row (mirrors the SQLite table)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayLog {
    pub id: i64,
    pub source_vcs: String,
    pub source_ref: String,
    pub target_vcs: String,
    pub target_ref: Option<String>,
    pub status: String,
    pub failure_msg: Option<String>,
    pub resolved_by: Option<String>,
    pub created_at: String,
    pub committed_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_policy_default_is_fail_stop() {
        assert_eq!(ReplayPolicy::default(), ReplayPolicy::FailStop);
    }

    #[test]
    fn file_change_kind_serialises() {
        let kind = FileChangeKind::Modify;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"modify\"");
    }

    #[test]
    fn unit_status_event_round_trip() {
        let event = UnitStatusEvent {
            source_ref: "svn:42".into(),
            status: UnitStatus::Committed,
            target_ref: Some("abc1234".into()),
            message: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: UnitStatusEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source_ref, "svn:42");
        assert_eq!(back.status, UnitStatus::Committed);
    }

    #[test]
    fn preview_result_aggregated_empty_by_default() {
        let result = PreviewResult {
            units: vec![],
            aggregated: vec![],
            stats: DiffStats {
                files_changed: 0,
                lines_added: 0,
                lines_removed: 0,
                binary_count: 0,
                conflict_risk_count: 0,
            },
        };
        assert!(result.aggregated.is_empty());
    }
}
