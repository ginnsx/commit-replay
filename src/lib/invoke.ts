import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { PreviewResult, ReplayUnitMeta, ValidationResult } from "./types";

export async function ping(): Promise<string> {
  return tauriInvoke("ping");
}

export async function listCommits(
  sourceVcs: string,
  url: string,
  limit: number,
): Promise<ReplayUnitMeta[]> {
  return tauriInvoke("list_commits", { source_vcs: sourceVcs, url, limit });
}

export async function buildPreview(
  sourceVcs: string,
  sourceUrl: string,
  sourceRefs: string[],
  targetWcPath: string,
): Promise<PreviewResult> {
  return tauriInvoke("build_preview", {
    source_vcs: sourceVcs,
    source_url: sourceUrl,
    source_refs: sourceRefs,
    target_wc_path: targetWcPath,
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
    source_ref: sourceRef,
    target_vcs: targetVcs,
    target_wc_path: targetWcPath,
    target_branch: targetBranch,
    message_template: messageTemplate,
  });
}

export async function commitResolved(
  sourceRef: string,
  targetWcPath: string,
  messageTemplate: string,
): Promise<ValidationResult> {
  return tauriInvoke("commit_resolved", {
    source_ref: sourceRef,
    target_wc_path: targetWcPath,
    message_template: messageTemplate,
  });
}

export async function openFileInSystem(path: string): Promise<void> {
  return tauriInvoke("open_file_in_system", { path });
}
