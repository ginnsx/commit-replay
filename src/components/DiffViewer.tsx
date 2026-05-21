import { DiffEditor } from "@monaco-editor/react";
import type { FileChange } from "../lib/types";

interface DiffViewerProps {
  file: FileChange | null;
}

export default function DiffViewer({ file }: DiffViewerProps) {
  if (!file) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-gray-500">
        选择左侧文件查看 diff
      </div>
    );
  }

  if (file.kind === "binary") {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 p-4 text-sm text-gray-400">
        <p className="font-mono text-gray-300">{file.target_path ?? file.path}</p>
        <p>二进制文件，无法显示文本 diff</p>
      </div>
    );
  }

  const original = file.before ?? "";
  const modified = file.after ?? "";
  const path = file.target_path ?? file.path;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b border-gray-800 px-3 py-2 text-xs text-gray-400">
        <span className="font-mono text-gray-200">{path}</span>
        <span className="ml-2 rounded bg-gray-800 px-1.5 py-0.5">{file.kind}</span>
        {file.conflict_risk === "high" && <span className="ml-2 text-orange-400">可能冲突</span>}
      </div>
      <div className="min-h-0 flex-1">
        <DiffEditor
          original={original}
          modified={modified}
          language="plaintext"
          theme="vs-dark"
          options={{
            readOnly: true,
            renderSideBySide: true,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
          }}
        />
      </div>
    </div>
  );
}
