import { beforeEach, describe, expect, it, vi } from "vitest";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { UPDATE_CHECK_INTERVAL_MS, checkForUpdate, installUpdate, shouldAutoCheck } from "./update";

vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: vi.fn(() => true) }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));

const checkMock = vi.mocked(check);
const relaunchMock = vi.mocked(relaunch);

describe("update service", () => {
  beforeEach(() => {
    checkMock.mockReset();
    relaunchMock.mockReset();
  });

  it("checks immediately when there is no successful check", () => {
    expect(shouldAutoCheck()).toBe(true);
  });

  it("waits twenty-four hours after a successful check", () => {
    const now = Date.parse("2026-08-04T00:00:00.000Z");
    expect(shouldAutoCheck(new Date(now - UPDATE_CHECK_INTERVAL_MS + 1).toISOString(), now)).toBe(
      false,
    );
    expect(shouldAutoCheck(new Date(now - UPDATE_CHECK_INTERVAL_MS).toISOString(), now)).toBe(true);
  });

  it("maps an available update without downloading it", async () => {
    const update = {
      version: "0.3.1",
      currentVersion: "0.3.0",
      body: "修复迁移预览问题",
      date: "2026-08-04T00:00:00Z",
    };
    checkMock.mockResolvedValue(update as never);

    await expect(checkForUpdate()).resolves.toMatchObject({
      info: {
        version: "0.3.1",
        currentVersion: "0.3.0",
        notes: "修复迁移预览问题",
      },
    });
  });

  it("returns no update when the release is current", async () => {
    checkMock.mockResolvedValue(null);

    await expect(checkForUpdate()).resolves.toBeNull();
  });

  it("keeps update check errors available to the caller", async () => {
    checkMock.mockRejectedValue(new Error("network unavailable"));

    await expect(checkForUpdate()).rejects.toThrow("network unavailable");
  });

  it("reports download progress before restarting", async () => {
    const downloadAndInstall = vi.fn(async (onEvent: (event: unknown) => void) => {
      onEvent({ event: "Started", data: { contentLength: 10 } });
      onEvent({ event: "Progress", data: { chunkLength: 4 } });
      onEvent({ event: "Progress", data: { chunkLength: 6 } });
      onEvent({ event: "Finished" });
    });
    const progress: number[] = [];

    await installUpdate(
      {
        info: { version: "0.3.1", currentVersion: "0.3.0" },
        update: { downloadAndInstall } as never,
      },
      ({ downloadedBytes }) => progress.push(downloadedBytes),
    );

    expect(progress).toEqual([0, 4, 10, 10]);
    expect(relaunchMock).toHaveBeenCalledOnce();
  });
});
