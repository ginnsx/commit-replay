import type { FileChangeView } from "../../lib/types";
import { StatusBadge } from "./Badges";

export function DiffView({ file }: { file?: FileChangeView | null }) {
  if (!file) {
    return (
      <div className="diff-panel">
        <div className="empty-state">
          <p>选择左侧文件查看变更详情</p>
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
      <div className="diff-body">
        {(file.diff ?? []).map((line, i) => (
          <div key={i} className={`diff-line ${line.type}`}>
            <span className={`diff-ln${line.old ? " old" : ""}`}>{line.old ?? ""}</span>
            <span className={`diff-ln${line.new ? " new" : ""}`}>{line.new ?? ""}</span>
            <span className="diff-code">
              {line.type === "add" ? "+ " : line.type === "del" ? "- " : "  "}
              {line.text}
            </span>
          </div>
        ))}
        {!file.diff?.length && (
          <div className="empty-state">
            <p>加载中…</p>
          </div>
        )}
      </div>
    </div>
  );
}
