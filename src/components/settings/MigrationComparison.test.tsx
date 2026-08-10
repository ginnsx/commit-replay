import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { compareMigrationFiles } from "../../lib/invoke";
import type { MigrationComparisonResult, MigrationRecord } from "../../lib/types";
import { MigrationComparison } from "./MigrationComparison";

vi.mock("../../lib/invoke", () => ({
  compareMigrationFiles: vi.fn(),
}));

const record: MigrationRecord = {
  id: "m1",
  completedAt: "2026-08-10 12:00",
  source: { name: "source", path: "D:\\source", type: "git", branch: "main" },
  target: { name: "target", path: "D:\\target", type: "git", branch: "main" },
  commits: [],
  files: [],
  comparisonFiles: [],
  conflictsResolved: 0,
  status: "success",
};

const completed: MigrationComparisonResult = {
  migrationId: "m1",
  comparedAt: "2026-08-10 12:01:00",
  available: true,
  summary: {
    total: 2,
    identical: 1,
    formatOnly: 0,
    different: 1,
    missing: 0,
    unreadable: 0,
  },
  files: [
    {
      id: "src/a.txt",
      sourcePath: "src/a.txt",
      targetPath: "src/a.txt",
      status: "different",
      contentKind: "text",
      diff: [
        { type: "del", old: 1, text: "old" },
        { type: "add", new: 1, text: "new" },
      ],
    },
    {
      id: "report.xlsx",
      sourcePath: "report.xlsx",
      targetPath: "report.xlsx",
      status: "identical",
      contentKind: "binary",
      sourceSize: 12,
      targetSize: 12,
      sourceSha256: "a".repeat(64),
      targetSha256: "a".repeat(64),
    },
  ],
};

describe("MigrationComparison", () => {
  it("手动对比并展示逐文件差异", async () => {
    const user = userEvent.setup();
    vi.mocked(compareMigrationFiles).mockResolvedValue(completed);
    render(<MigrationComparison record={record} autoRun={false} />);

    await user.click(screen.getByRole("button", { name: "开始对比" }));

    await waitFor(() => expect(compareMigrationFiles).toHaveBeenCalledWith("m1"));
    expect(screen.getByText("2 个文件")).toBeVisible();
    expect(screen.getByText("1 内容不同")).toBeVisible();
    expect(screen.getByText("src/a.txt ↔ src/a.txt")).toBeVisible();
  });

  it("自动执行并展示旧记录不可安全恢复的原因", async () => {
    vi.mocked(compareMigrationFiles).mockResolvedValue({
      migrationId: "m1",
      comparedAt: "2026-08-10 12:01:00",
      available: false,
      reason: "旧记录缺少可靠路径映射，无法安全对比",
      summary: { total: 0, identical: 0, formatOnly: 0, different: 0, missing: 0, unreadable: 0 },
      files: [],
    });
    render(<MigrationComparison record={record} autoRun />);

    await waitFor(() => expect(compareMigrationFiles).toHaveBeenCalledWith("m1"));
    expect(screen.getByText("无法安全对比此记录")).toBeVisible();
    expect(screen.getByText("旧记录缺少可靠路径映射，无法安全对比")).toBeVisible();
  });
});
