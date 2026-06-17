import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  AppErrorPayload,
  CommitListItem,
  Editor,
  FileChangeView,
  IntegrationItemView,
  IntegrationPlanResult,
  MigrationMode,
  MigrationRecord,
  MigrationResult,
  PathMapping,
  PreviewMetaResult,
  Repo,
  RepoInput,
  GitRepoInfo,
  SvnWcInfo,
} from "./types";

function parseError(e: unknown): AppErrorPayload {
  if (typeof e === "string") {
    try {
      const parsed = JSON.parse(e) as AppErrorPayload;
      if (parsed.message) return parsed;
    } catch {
      return { code: "unknown", message: e, retryable: false };
    }
    return { code: "unknown", message: e, retryable: false };
  }
  if (e && typeof e === "object" && "message" in e) {
    return e as AppErrorPayload;
  }
  return { code: "unknown", message: String(e), retryable: false };
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (e) {
    throw parseError(e);
  }
}

export async function ping(): Promise<string> {
  return invoke("ping");
}

export async function listRepos(): Promise<Repo[]> {
  const rows = await invoke<Array<Record<string, unknown>>>("relay_list_repos");
  return rows.map(normalizeRepo);
}

export async function saveRepo(input: RepoInput): Promise<Repo> {
  const row = await invoke<Record<string, unknown>>("relay_save_repo", {
    input: {
      id: input.id ?? null,
      name: input.name,
      path: input.path,
      type: input.type,
      branch: input.branch,
      svn_user: input.svnUser ?? null,
      svn_pass: input.svnPass ?? null,
      path_mappings: input.pathMappings,
    },
  });
  return normalizeRepo(row);
}

export async function deleteRepo(id: string): Promise<void> {
  return invoke("relay_delete_repo", { id });
}

export async function listEditors(): Promise<Editor[]> {
  return invoke("relay_list_editors");
}

export async function saveEditor(editor: Editor): Promise<Editor> {
  return invoke("relay_save_editor", { editor });
}

export async function deleteEditor(id: string): Promise<void> {
  return invoke("relay_delete_editor", { id });
}

export async function getDefaultEditorId(): Promise<string> {
  return invoke("relay_get_default_editor");
}

export async function setDefaultEditor(id: string): Promise<void> {
  return invoke("relay_set_default_editor", { id });
}

export async function listMigrations(): Promise<MigrationRecord[]> {
  const rows = await invoke<Array<Record<string, unknown>>>("relay_list_migrations");
  return rows.map(normalizeMigration);
}

export async function getMigration(id: string): Promise<MigrationRecord | null> {
  const row = await invoke<Record<string, unknown> | null>("relay_get_migration", { id });
  return row ? normalizeMigration(row) : null;
}

export async function pickFolder(): Promise<string | null> {
  return invoke<string | null>("relay_pick_folder");
}

export async function probeSvnWc(path: string): Promise<SvnWcInfo> {
  const r = await invoke<Record<string, unknown>>("relay_probe_svn_wc", { path });
  return {
    branch: String(r.branch),
    relativeUrl: String(r.relative_url),
    url: String(r.url),
  };
}

export async function probeGitRepo(path: string): Promise<GitRepoInfo> {
  const r = await invoke<Record<string, unknown>>("relay_probe_git_repo", { path });
  return {
    branch: String(r.branch),
    branches: (r.branches as string[]) ?? [],
    remoteUrl: r.remote_url ? String(r.remote_url) : undefined,
  };
}

export async function listRepoCommits(
  repoId: string,
  limit = 50,
  beforeRevision: number | null = null,
): Promise<CommitListItem[]> {
  const rows = await invoke<Array<Record<string, unknown>>>("list_repo_commits", {
    repoId,
    limit,
    beforeRevision,
  });
  return rows.map((r) => ({
    id: String(r.id),
    hash: String(r.hash),
    msg: String(r.msg),
    author: String(r.author),
    date: String(r.date),
    files: Number(r.files),
    sourceRef: String(r.source_ref),
  }));
}

export async function validateMigrationCombo(
  sourceId: string,
  targetId: string,
): Promise<boolean> {
  return invoke("validate_migration_combo", { sourceId, targetId });
}

function mappingArgs(mappings: PathMapping[]) {
  return mappings.map((m) => ({ from: m.from, to: m.to }));
}

export async function buildPreviewMeta(
  sourceId: string,
  targetId: string,
  sourceRefs: string[],
  pathMappings: PathMapping[],
): Promise<PreviewMetaResult> {
  const r = await invoke<Record<string, unknown>>("build_preview_meta", {
    sourceId,
    targetId,
    sourceRefs,
    pathMappings: mappingArgs(pathMappings),
  });
  return {
    files: (r.files as FileChangeView[]) ?? [],
    adds: Number(r.adds),
    mods: Number(r.mods),
    dels: Number(r.dels),
    totalAdditions: Number(r.total_additions),
    totalDeletions: Number(r.total_deletions),
  };
}

export async function getFileDiff(
  sourceId: string,
  targetId: string,
  sourceRefs: string[],
  filePath: string,
  pathMappings: PathMapping[],
): Promise<FileChangeView> {
  return invoke("get_file_diff", {
    sourceId,
    targetId,
    sourceRefs,
    filePath,
    pathMappings: mappingArgs(pathMappings),
  });
}

export async function buildIntegrationPlan(
  sourceId: string,
  targetId: string,
  sourceRefs: string[],
  pathMappings: PathMapping[],
  mode: MigrationMode = "incremental_first",
): Promise<IntegrationPlanResult> {
  return invoke("build_integration_plan_cmd", {
    sourceId,
    targetId,
    sourceRefs,
    pathMappings: mappingArgs(pathMappings),
    mode,
  });
}

export async function detectConflicts(
  sourceId: string,
  targetId: string,
  sourceRefs: string[],
  pathMappings: PathMapping[],
): Promise<IntegrationItemView[]> {
  return invoke("detect_conflicts", {
    sourceId,
    targetId,
    sourceRefs,
    pathMappings: mappingArgs(pathMappings),
  });
}

export async function executeMigration(
  sourceId: string,
  targetId: string,
  sourceRefs: string[],
  conflictsResolved: number,
  pathMappings: PathMapping[],
  migrationMode: MigrationMode = "incremental_first",
  acceptedReview: string[] = [],
  resolvedBlocked: string[] = [],
): Promise<MigrationResult> {
  const r = await invoke<Record<string, unknown>>("execute_migration", {
    sourceId,
    targetId,
    sourceRefs,
    conflictsResolved,
    pathMappings: mappingArgs(pathMappings),
    migrationMode,
    acceptedReview,
    resolvedBlocked,
  });
  return {
    migrationId: String(r.migration_id),
    commitsApplied: Number(r.commits_applied),
    filesChanged: Number(r.files_changed),
  };
}

export async function openFileInEditor(editorId: string, filePath: string): Promise<void> {
  return invoke("open_file_in_editor", { editorId, filePath });
}

function normalizeRepo(r: Record<string, unknown>): Repo {
  return {
    id: String(r.id),
    name: String(r.name),
    path: String(r.path),
    type: (r.type ?? r.repo_type ?? "git") as Repo["type"],
    branch: String(r.branch),
    lastUsed: r.last_used ? String(r.last_used) : undefined,
    svnUser: r.svn_user ? String(r.svn_user) : undefined,
    hasSvnPass: Boolean(r.has_svn_pass),
    pathMappings: (r.path_mappings as Repo["pathMappings"]) ?? [],
  };
}

function normalizeMigration(r: Record<string, unknown>): MigrationRecord {
  return {
    id: String(r.id),
    completedAt: String(r.completed_at ?? r.completedAt),
    source: r.source as MigrationRecord["source"],
    target: r.target as MigrationRecord["target"],
    commits: ((r.commits as Array<Record<string, unknown>>) ?? []).map((c) => ({
      id: String(c.id),
      hash: String(c.hash),
      msg: String(c.msg),
      author: String(c.author),
      date: String(c.date),
      files: Number(c.files),
    })),
    files: (r.files as FileChangeView[]) ?? [],
    conflictsResolved: Number(r.conflicts_resolved ?? r.conflictsResolved ?? 0),
    status: String(r.status),
  };
}
