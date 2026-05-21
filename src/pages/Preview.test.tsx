import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PreviewResult } from "../lib/types";

vi.mock("@monaco-editor/react", () => ({
  DiffEditor: () => <div data-testid="diff-editor" />,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import Preview from "./Preview";
import { useConnectionStore } from "../store/connectionStore";
import { useSelectionStore } from "../store/selection";

const samplePreview: PreviewResult = {
  units: [],
  aggregated: [
    {
      path: "/trunk/a.txt",
      target_path: "a.txt",
      kind: "modify",
      old_path: null,
      before: "old",
      after: "new",
      patch: "@@\n-old\n+new\n",
      conflict_risk: "low",
    },
  ],
  stats: {
    files_changed: 1,
    lines_added: 1,
    lines_removed: 1,
    binary_count: 0,
    conflict_risk_count: 0,
  },
};

function renderPreview() {
  return render(
    <MemoryRouter>
      <Preview />
    </MemoryRouter>,
  );
}

describe("Preview", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    useConnectionStore.setState({
      svn: {
        url: "https://svn.example.com/repo",
        username: "",
        password: "",
        limit: 20,
      },
      target: { wcPath: "C:\\git\\repo", mappings: [{ from: "/trunk", to: "." }] },
    });
    useSelectionStore.setState({ selectedRefs: new Set(["svn:101"]) });
  });

  it("requires target wc path", async () => {
    useConnectionStore.setState({
      target: { wcPath: "", mappings: [{ from: "/trunk", to: "." }] },
    });
    renderPreview();
    fireEvent.click(screen.getByRole("button", { name: "生成预览" }));
    expect(await screen.findByText("请填写目标 Git 工作区路径")).toBeInTheDocument();
  });

  it("shows preview stats and file tree after build", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(samplePreview);
    renderPreview();
    fireEvent.click(screen.getByRole("button", { name: "生成预览" }));

    await waitFor(() => {
      expect(screen.getByText(/1 文件/)).toBeInTheDocument();
    });
    expect(screen.getByTestId("diff-editor")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith(
      "build_preview",
      expect.objectContaining({
        sourceVcs: "svn",
        targetWcPath: "C:\\git\\repo",
        sourceRefs: ["svn:101"],
      }),
    );
  });
});
