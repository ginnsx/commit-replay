import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Repo } from "../../lib/types";
import { ConflictWorkspace } from "./ConflictWorkspace";
import { MigrationSummary } from "./MigrationSummary";

const source: Repo = {
  id: "source",
  name: "payment-service",
  path: "D:\\Projects\\payment-service",
  type: "git",
  branch: "main",
  pathMappings: [],
};

const target: Repo = {
  id: "target",
  name: "legacy-billing",
  path: "D:\\SVN\\legacy-billing",
  type: "svn",
  branch: "trunk",
  pathMappings: [],
};

describe("迁移代码阅读体验", () => {
  it("默认展示紧凑摘要，并可展开完整信息", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();

    const { rerender } = render(
      <MigrationSummary
        source={source}
        target={target}
        commitCount={2}
        fileCount={5}
        migrationMode="incremental_first"
        squashCommits={false}
        mappings={[{ from: ".", to: "." }]}
        customMapping={false}
        expanded={false}
        onToggle={onToggle}
      />,
    );

    expect(screen.getByText("payment-service → legacy-billing")).toBeVisible();
    expect(screen.getByText("2 条提交")).toBeVisible();
    expect(screen.queryByText("目标路径")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "展开详情" }));
    expect(onToggle).toHaveBeenCalledOnce();

    rerender(
      <MigrationSummary
        source={source}
        target={target}
        commitCount={2}
        fileCount={5}
        migrationMode="incremental_first"
        squashCommits={false}
        mappings={[{ from: ".", to: "." }]}
        customMapping={false}
        expanded
        onToggle={onToggle}
      />,
    );

    expect(screen.getByText("目标路径")).toBeVisible();
    expect(screen.getByText("D:\\SVN\\legacy-billing")).toBeVisible();
  });

  it("提供专注预览和独立代码字号操作", async () => {
    const user = userEvent.setup();
    const onToggleFocusPreview = vi.fn();
    const onCodeZoomChange = vi.fn();

    render(
      <ConflictWorkspace
        items={[]}
        source={source}
        target={target}
        commitCount={2}
        fileCount={5}
        mappings={[{ from: ".", to: "." }]}
        customMapping={false}
        migrationMode="incremental_first"
        onMigrationModeChange={vi.fn()}
        squashCommits={false}
        onSquashCommitsChange={vi.fn()}
        squashCommitMessage=""
        onSquashCommitMessageChange={vi.fn()}
        autoOkCount={0}
        reviewCount={0}
        blockedCount={0}
        activeId={null}
        acceptedReviewIds={new Set()}
        resolvedBlockedIds={new Set()}
        editors={[]}
        selectedEditorId="vscode"
        onSelectEditor={vi.fn()}
        onBrowseEditor={vi.fn()}
        onSelect={vi.fn()}
        onOpenWithEditor={vi.fn()}
        onAcceptReview={vi.fn()}
        onMarkResolved={vi.fn()}
        onAcceptAllReview={vi.fn()}
        canExecute
        summaryExpanded={false}
        onToggleSummary={vi.fn()}
        focusPreview={false}
        onToggleFocusPreview={onToggleFocusPreview}
        codeZoom={1}
        onCodeZoomChange={onCodeZoomChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: "放大代码" }));
    expect(onCodeZoomChange).toHaveBeenCalledWith(1.1);

    await user.click(screen.getByRole("button", { name: "专注预览" }));
    expect(onToggleFocusPreview).toHaveBeenCalledOnce();
  });
});
