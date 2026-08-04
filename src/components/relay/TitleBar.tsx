import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseEvent } from "react";
import { IconClose, IconMax, IconMin } from "./icons";

export function TitleBar({
  updateVersion,
  onOpenUpdate,
}: {
  updateVersion?: string;
  onOpenUpdate: () => void;
}) {
  const win = getCurrentWindow();

  const onDragMouseDown = (e: MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    if (e.detail === 2) {
      void win.toggleMaximize();
      return;
    }
    void win.startDragging();
  };

  return (
    <div className="titlebar">
      <div className="titlebar-drag" onMouseDown={onDragMouseDown}>
        <div className="app-logo">R</div>
        <div className="app-title">
          <strong>Relay</strong> — 跨仓库提交迁移
        </div>
      </div>
      <div className="win-controls">
        {updateVersion ? (
          <button type="button" className="titlebar-update" onClick={onOpenUpdate}>
            发现新版本 v{updateVersion}
          </button>
        ) : null}
        <button type="button" className="win-btn" aria-label="最小化" onClick={() => void win.minimize()}>
          <IconMin />
        </button>
        <button type="button" className="win-btn" aria-label="最大化" onClick={() => void win.toggleMaximize()}>
          <IconMax />
        </button>
        <button type="button" className="win-btn close" aria-label="关闭" onClick={() => void win.close()}>
          <IconClose />
        </button>
      </div>
    </div>
  );
}
