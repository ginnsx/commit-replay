import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReplayUnitMeta } from "../lib/types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import CommitList from "./CommitList";
import { useConnectionStore } from "../store/connectionStore";
import { useSelectionStore } from "../store/selection";

const sampleCommits: ReplayUnitMeta[] = [
  {
    source_ref: "svn:101",
    author: "bob",
    date: "2026-05-11T09:30:00.000000Z",
    message: "fix typo",
    changed_paths_count: 1,
  },
  {
    source_ref: "svn:100",
    author: "alice",
    date: "2026-05-10T08:00:00.123456Z",
    message: "initial",
    changed_paths_count: 2,
  },
];

describe("CommitList", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    useConnectionStore.setState({
      svn: { url: "https://svn.example.com/repo", username: "u", password: "p", limit: 10 },
    });
    useSelectionStore.setState({ selectedRefs: new Set() });
  });

  it("shows validation error when URL is empty", async () => {
    useConnectionStore.setState({
      svn: { url: "", username: "", password: "", limit: 20 },
    });
    render(<CommitList />);
    fireEvent.click(screen.getByRole("button", { name: "拉取提交" }));
    expect(await screen.findByText("请填写 SVN 仓库 URL")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("renders commits after fetch", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(sampleCommits);
    render(<CommitList />);
    fireEvent.click(screen.getByRole("button", { name: "拉取提交" }));

    await waitFor(() => {
      expect(screen.getByText("r101")).toBeInTheDocument();
    });
    expect(screen.getByText("bob")).toBeInTheDocument();
    expect(screen.getByText("fix typo")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("list_commits", {
      sourceVcs: "svn",
      url: "https://svn.example.com/repo",
      limit: 10,
      username: "u",
      password: "p",
    });
  });

  it("toggles row selection", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(sampleCommits);
    render(<CommitList />);
    fireEvent.click(screen.getByRole("button", { name: "拉取提交" }));
    await waitFor(() => expect(screen.getByText("r101")).toBeInTheDocument());

    const rowCheckbox = screen.getByRole("checkbox", { name: "选择 svn:101" });
    fireEvent.click(rowCheckbox);
    expect(screen.getByText("已选 1 条")).toBeInTheDocument();

    fireEvent.click(rowCheckbox);
    expect(screen.queryByText("已选 1 条")).not.toBeInTheDocument();
  });

  it("shows error when invoke fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("svn connection refused");
    render(<CommitList />);
    fireEvent.click(screen.getByRole("button", { name: "拉取提交" }));
    expect(await screen.findByText("svn connection refused")).toBeInTheDocument();
  });
});
