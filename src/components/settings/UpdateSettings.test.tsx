import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UpdateSettings } from "./UpdateSettings";

describe("UpdateSettings", () => {
  it("allows a manual check even if the last check was recent", () => {
    const onCheck = vi.fn();
    render(
      <UpdateSettings
        appVersion="0.3.0"
        status="latest"
        lastCheckedAt={new Date().toISOString()}
        migrating={false}
        supported
        onCheck={onCheck}
        onInstall={vi.fn()}
        onLater={vi.fn()}
      />,
    );

    screen.getByRole("button", { name: "检查更新" }).click();
    expect(onCheck).toHaveBeenCalledOnce();
  });

  it("prevents installation while a migration is running", () => {
    render(
      <UpdateSettings
        appVersion="0.3.0"
        status="available"
        availableUpdate={{ version: "0.3.1", currentVersion: "0.3.0" }}
        migrating
        supported
        onCheck={vi.fn()}
        onInstall={vi.fn()}
        onLater={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "立即更新" })).toBeDisabled();
    expect(screen.getByText("迁移执行中，完成后才能安装更新。")).toBeInTheDocument();
  });
});
