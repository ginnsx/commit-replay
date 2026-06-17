import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { FileChangeView } from "../../lib/types";
import { splitFilePath } from "../../lib/constants";
import { StatusBadge } from "./Badges";

export function FileTree({
  files,
  activeId,
  onSelect,
}: {
  files: FileChangeView[];
  activeId: string | null;
  onSelect: (id: string) => void;
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const counts = { add: 0, mod: 0, del: 0 };
  files.forEach((f) => counts[f.status]++);

  const virtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 48,
    overscan: 8,
  });

  return (
    <div className="file-tree">
      <div className="file-tree-header">
        <span>文件变更</span>
        <span>
          <span className="badge badge-add">+{counts.add}</span>{" "}
          <span className="badge badge-mod">~{counts.mod}</span>{" "}
          <span className="badge badge-del">−{counts.del}</span>
        </span>
      </div>
      <div ref={parentRef} style={{ overflow: "auto", flex: 1, minHeight: 0 }}>
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((item) => {
            const f = files[item.index];
            const { dir, name } = splitFilePath(f.path);
            return (
              <div
                key={f.id}
                className={`file-item${activeId === f.id ? " active" : ""}`}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${item.start}px)`,
                }}
                onClick={() => onSelect(f.id)}
              >
                <StatusBadge status={f.status} />
                <div className="file-item-path" title={f.path}>
                  <span className="file-item-name">{name}</span>
                  {dir ? <span className="file-item-dir">{dir}</span> : null}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
