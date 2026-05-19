import { describe, it, expect } from "vitest";
import type { FileChange, PreviewResult, ReplayUnitMeta, UnitStatusEvent } from "./types";

describe("TypeScript model types", () => {
  it("ReplayUnitMeta has required fields", () => {
    const meta: ReplayUnitMeta = {
      source_ref: "svn:42",
      author: "alice",
      date: "2026-05-19T10:00:00Z",
      message: "fix: typo",
      changed_paths_count: 3,
    };
    expect(meta.source_ref).toBe("svn:42");
    expect(meta.changed_paths_count).toBe(3);
  });

  it("FileChange allows null optional fields", () => {
    const fc: FileChange = {
      path: "/trunk/src/main.rs",
      target_path: null,
      kind: "modify",
      old_path: null,
      before: null,
      after: null,
      patch: null,
      conflict_risk: null,
    };
    expect(fc.kind).toBe("modify");
    expect(fc.target_path).toBeNull();
  });

  it("PreviewResult initialises with empty aggregated", () => {
    const result: PreviewResult = {
      units: [],
      aggregated: [],
      stats: {
        files_changed: 0,
        lines_added: 0,
        lines_removed: 0,
        binary_count: 0,
        conflict_risk_count: 0,
      },
    };
    expect(result.aggregated).toHaveLength(0);
    expect(result.stats.files_changed).toBe(0);
  });

  it("UnitStatusEvent captures all status variants", () => {
    const event: UnitStatusEvent = {
      source_ref: "svn:100",
      status: "committed",
      target_ref: "abc1234",
      message: null,
    };
    expect(event.status).toBe("committed");
  });
});
