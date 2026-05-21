import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { ChangeSet, PreviewResult, ReplayUnitMeta, ValidationResult } from "./types";

/**
 * Tauri 2 maps Rust snake_case command args to camelCase in invoke payloads.
 * Use sourceVcs, not source_vcs.
 */

export async function ping(): Promise<string> {
  return tauriInvoke("ping");
}

export async function listCommits(
  sourceVcs: string,
  url: string,
  limit: number,
  username?: string,
  password?: string,
): Promise<ReplayUnitMeta[]> {
  return tauriInvoke("list_commits", {
    sourceVcs,
    url,
    limit,
    username: username ?? null,
    password: password ?? null,
  });
}

export async function loadChangeset(
  sourceVcs: string,
  url: string,
  sourceRef: string,
  username?: string,
  password?: string,
): Promise<ChangeSet> {
  return tauriInvoke("load_changeset", {
    sourceVcs,
    url,
    sourceRef,
    username: username ?? null,
    password: password ?? null,
  });
}

export async function buildPreview(
  sourceVcs: string,
  sourceUrl: string,
  sourceRefs: string[],
  targetWcPath: string,
): Promise<PreviewResult> {
  return tauriInvoke("build_preview", {
    sourceVcs,
    sourceUrl,
    sourceRefs,
    targetWcPath,
  });
}

export async function applyUnit(
  sourceRef: string,
  targetVcs: string,
  targetWcPath: string,
  targetBranch: string,
  messageTemplate: string,
): Promise<string> {
  return tauriInvoke("apply_unit", {
    sourceRef,
    targetVcs,
    targetWcPath,
    targetBranch,
    messageTemplate,
  });
}

export async function commitResolved(
  sourceRef: string,
  targetWcPath: string,
  messageTemplate: string,
): Promise<ValidationResult> {
  return tauriInvoke("commit_resolved", {
    sourceRef,
    targetWcPath,
    messageTemplate,
  });
}

export async function openFileInSystem(path: string): Promise<void> {
  return tauriInvoke("open_file_in_system", { path });
}
