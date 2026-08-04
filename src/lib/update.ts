import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import type { AvailableUpdate } from "./types";

export const UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

export type UpdateStatus =
  | "idle"
  | "checking"
  | "latest"
  | "available"
  | "downloading"
  | "failed"
  | "unsupported";

export interface PendingUpdate {
  info: AvailableUpdate;
  update: Update;
}

export interface DownloadProgress {
  downloadedBytes: number;
  totalBytes?: number;
}

export function isUpdateSupported(): boolean {
  return !import.meta.env.DEV && isTauri();
}

export function shouldAutoCheck(lastCheckedAt?: string, now = Date.now()): boolean {
  if (!lastCheckedAt) return true;
  const checkedAt = Date.parse(lastCheckedAt);
  return Number.isNaN(checkedAt) || now - checkedAt >= UPDATE_CHECK_INTERVAL_MS;
}

export async function getAppVersion(): Promise<string> {
  return getVersion();
}

export async function checkForUpdate(): Promise<PendingUpdate | null> {
  const update = await check({ timeout: 10_000 });
  if (!update) return null;
  return {
    info: {
      version: update.version,
      currentVersion: update.currentVersion,
      notes: update.body,
      date: update.date,
    },
    update,
  };
}

export async function installUpdate(
  pending: PendingUpdate,
  onProgress: (progress: DownloadProgress) => void,
): Promise<void> {
  let downloadedBytes = 0;
  let totalBytes: number | undefined;
  await pending.update.downloadAndInstall(
    (event) => {
      if (event.event === "Started") {
        totalBytes = event.data.contentLength;
      } else if (event.event === "Progress") {
        downloadedBytes += event.data.chunkLength;
      }
      onProgress({ downloadedBytes, totalBytes });
    },
    { timeout: 30_000 },
  );
  await relaunch();
}
