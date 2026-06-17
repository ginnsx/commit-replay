// Relay types — aligned with Rust store/models.rs

export type VcsKind = "git" | "svn";

export interface PathMapping {
  from: string;
  to: string;
}

export interface SvnWcInfo {
  branch: string;
  relativeUrl: string;
  url: string;
}

export interface Repo {
  id: string;
  name: string;
  path: string;
  type: VcsKind;
  branch: string;
  lastUsed?: string;
  svnUser?: string;
  hasSvnPass?: boolean;
  pathMappings: PathMapping[];
}

export interface RepoInput {
  id?: string;
  name: string;
  path: string;
  type: VcsKind;
  branch: string;
  svnUser?: string;
  svnPass?: string;
  pathMappings: PathMapping[];
}

export interface Editor {
  id: string;
  name: string;
  exe: string;
  kind: string;
  custom: boolean;
}

export type FileStatus = "add" | "mod" | "del";

export type DiffLineType = "ctx" | "add" | "del";

export interface DiffLine {
  type: DiffLineType;
  old?: number;
  new?: number;
  text: string;
}

export interface FileChangeView {
  id: string;
  path: string;
  status: FileStatus;
  additions: number;
  deletions: number;
  diff?: DiffLine[];
}

export type MigrationMode = "incremental_first" | "commit_result" | "strict_replay";

export type IntegrationStatus = "auto_ok" | "review" | "blocked";

export type IntegrationStrategy = "skip" | "apply_patch" | "write_after" | "manual_merge";

export interface IntegrationItemView {
  id: string;
  path: string;
  status: FileStatus;
  integrationStatus: IntegrationStatus;
  strategy: IntegrationStrategy;
  reason: string;
  overlapLines?: [number, number];
  before: string[];
  after: string[];
}

export interface IntegrationPlanResult {
  mode: MigrationMode;
  autoOkCount: number;
  reviewCount: number;
  blockedCount: number;
  items: IntegrationItemView[];
}

/** @deprecated use IntegrationItemView */
export type ConflictView = IntegrationItemView;

export interface CommitListItem {
  id: string;
  hash: string;
  msg: string;
  author: string;
  date: string;
  files: number;
  sourceRef: string;
}

export interface CommitSnapshot {
  id: string;
  hash: string;
  msg: string;
  author: string;
  date: string;
  files: number;
}

export interface RepoSnapshot {
  name: string;
  path: string;
  type: VcsKind;
  branch: string;
}

export interface MigrationRecord {
  id: string;
  completedAt: string;
  source: RepoSnapshot;
  target: RepoSnapshot;
  commits: CommitSnapshot[];
  files: FileChangeView[];
  conflictsResolved: number;
  status: string;
}

export interface PreviewMetaResult {
  files: FileChangeView[];
  adds: number;
  mods: number;
  dels: number;
  totalAdditions: number;
  totalDeletions: number;
}

export interface MigrationResult {
  migrationId: string;
  commitsApplied: number;
  filesChanged: number;
}

export interface AppErrorPayload {
  code: string;
  message: string;
  retryable: boolean;
}

export type WizardStep = "source" | "commits" | "preview" | "target" | "migrate";

// Legacy VCS types (preview pipeline)
export interface ReplayUnitMeta {
  source_ref: string;
  author: string;
  date: string;
  message: string;
  changed_paths_count: number;
}

export type FileChangeKind = "add" | "modify" | "delete" | "rename" | "binary";

export interface FileChange {
  path: string;
  target_path: string | null;
  kind: FileChangeKind;
  old_path: string | null;
  before: string | null;
  after: string | null;
  patch: string | null;
  conflict_risk: "low" | "high" | null;
}

export interface PreviewResult {
  units: { meta: ReplayUnitMeta; files: FileChange[] }[];
  aggregated: FileChange[];
  stats: {
    files_changed: number;
    lines_added: number;
    lines_removed: number;
    binary_count: number;
    conflict_risk_count: number;
  };
}

export type UnitStatus = "pending" | "applying" | "committed" | "failed" | "skipped" | "aborted";

export interface UnitStatusEvent {
  source_ref: string;
  status: UnitStatus;
  target_ref: string | null;
  message: string | null;
}
