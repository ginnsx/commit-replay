// TypeScript mirror of src-tauri/src/model.rs
// Keep these in sync with the Rust structs.

export type VcsKind = "svn" | "git";

export interface PathMapping {
  from: string;
  to: string;
}

export type ReplayPolicy = "fail_stop" | "skip_failed" | "manual_on_fail";

export interface ReplayUnitMeta {
  source_ref: string;
  author: string;
  date: string;
  message: string;
  changed_paths_count: number;
}

export type FileChangeKind = "add" | "modify" | "delete" | "rename" | "binary";

export type ConflictRisk = "low" | "high";

export interface FileChange {
  path: string;
  target_path: string | null;
  kind: FileChangeKind;
  old_path: string | null;
  before: string | null;
  after: string | null;
  patch: string | null;
  conflict_risk: ConflictRisk | null;
}

export interface ChangeSet {
  meta: ReplayUnitMeta;
  files: FileChange[];
}

export interface DiffStats {
  files_changed: number;
  lines_added: number;
  lines_removed: number;
  binary_count: number;
  conflict_risk_count: number;
}

export interface PreviewUnit {
  meta: ReplayUnitMeta;
  files: FileChange[];
}

export interface PreviewResult {
  units: PreviewUnit[];
  aggregated: FileChange[];
  stats: DiffStats;
}

export type ApplyStatus = "ok" | "conflict" | "missing_base" | "error";

export interface ApplyResult {
  status: ApplyStatus;
  message: string | null;
  failed_paths: string[];
}

export interface ValidationIssue {
  path: string;
  reason: string;
}

export interface ValidationResult {
  passed: boolean;
  issues: ValidationIssue[];
}

export type UnitStatus = "pending" | "applying" | "committed" | "failed" | "skipped" | "aborted";

export interface UnitStatusEvent {
  source_ref: string;
  status: UnitStatus;
  target_ref: string | null;
  message: string | null;
}

export interface ReplayLog {
  id: number;
  source_vcs: string;
  source_ref: string;
  target_vcs: string;
  target_ref: string | null;
  status: string;
  failure_msg: string | null;
  resolved_by: string | null;
  created_at: string;
  committed_at: string | null;
}
