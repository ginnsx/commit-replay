import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { FileChangeView } from "../../lib/types";
import { StatusBadge } from "./Badges";

const DIFF_LINE_HEIGHT = 20;

export function DiffView({
  file,
  loading = false,
}: {
  file?: FileChangeView | null;
  loading?: boolean;
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const lines = file?.diff ?? [];

  const virtualizer = useVirtualizer({
    count: lines.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => DIFF_LINE_HEIGHT,
    overscan: 24,
  });

  if (!file) {
    return (
      <div className="diff-panel">
        <div className="empty-state">
          <p>{loading ? "加载变更中…" : "选择左侧文件查看变更详情"}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="diff-panel">
      <div className="diff-header">
        <span>{file.path}</span>
        <StatusBadge status={file.status} />
      </div>
      <div ref={parentRef} className="diff-body">
        {lines.length === 0 ? (
          <div className="empty-state">
            <p>加载中…</p>
          </div>
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((item) => {
              const line = lines[item.index];
              return (
                <div
                  key={item.index}
                  className={`diff-line ${line.type}`}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    transform: `translateY(${item.start}px)`,
                  }}
                >
                  <span className={`diff-ln${line.old ? " old" : ""}`}>{line.old ?? ""}</span>
                  <span className={`diff-ln${line.new ? " new" : ""}`}>{line.new ?? ""}</span>
                  <span className="diff-code">
                    {line.type === "add" ? "+ " : line.type === "del" ? "- " : "  "}
                    {line.text}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
