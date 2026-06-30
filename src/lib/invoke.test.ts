import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { buildSourcePreviewMeta } from "./invoke";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(tauriInvoke);

describe("invoke wrappers", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("buildSourcePreviewMeta calls source-only preview command", async () => {
    invokeMock.mockResolvedValue({
      files: [],
      adds: 1,
      mods: 2,
      dels: 3,
      total_additions: 4,
      total_deletions: 5,
    });

    const result = await buildSourcePreviewMeta("source-1", ["git:abc"]);

    expect(invokeMock).toHaveBeenCalledWith("build_source_preview_meta", {
      sourceId: "source-1",
      sourceRefs: ["git:abc"],
    });
    expect(result).toMatchObject({
      files: [],
      adds: 1,
      mods: 2,
      dels: 3,
      totalAdditions: 4,
      totalDeletions: 5,
    });
  });
});
