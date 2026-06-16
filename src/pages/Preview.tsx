import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import DiffViewer from "../components/DiffViewer";
import FileTree, { type FileTreeMode } from "../components/FileTree";
import { buildPreview } from "../lib/invoke";
import type { FileChange, PreviewResult } from "../lib/types";
import { useConnectionStore } from "../store/connectionStore";
import { useSelectionStore } from "../store/selection";

function sortRefs(refs: string[]): string[] {
  return [...refs].sort((a, b) => {
    const ra = Number(a.replace(/^svn:/, "")) || 0;
    const rb = Number(b.replace(/^svn:/, "")) || 0;
    return ra - rb;
  });
}

export default function Preview() {
  const { svn, target, setTarget, setMapping, addMapping, removeMapping } = useConnectionStore();
  const { selectedRefs } = useSelectionStore();
  const refs = useMemo(() => sortRefs([...selectedRefs]), [selectedRefs]);

  const [preview, setPreview] = useState<PreviewResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [treeMode, setTreeMode] = useState<FileTreeMode>("aggregated");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);

  const selectedFile: FileChange | null = useMemo(() => {
    if (!preview || !selectedPath) return null;
    const pool =
      treeMode === "aggregated" ? preview.aggregated : preview.units.flatMap((u) => u.files);
    return pool.find((f) => (f.target_path ?? f.path) === selectedPath) ?? null;
  }, [preview, selectedPath, treeMode]);

  const runPreview = async () => {
    if (refs.length === 0) {
      setError("请先在提交列表勾选至少一条记录");
      return;
    }
    if (!svn.url.trim()) {
      setError("请先在提交列表配置 SVN URL");
      return;
    }
    if (!target.wcPath.trim()) {
      setError("请填写目标 Git 工作区路径");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const result = await buildPreview(
        "svn",
        svn.url.trim(),
        refs,
        target.wcPath.trim(),
        target.mappings.filter((m) => m.from.trim()),
        svn.username || undefined,
        svn.password || undefined,
      );
      setPreview(result);
      const first = result.aggregated[0];
      setSelectedPath(first ? (first.target_path ?? first.path) : null);
    } catch (e) {
      setError(String(e));
      setPreview(null);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex h-[calc(100vh-49px)] flex-col">
      <div className="shrink-0 border-b border-gray-800 p-4">
        <div className="mx-auto flex max-w-6xl flex-col gap-4">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="text-xl font-bold text-gray-100">变更预览</h1>
              <p className="text-sm text-gray-400">
                已选 {refs.length} 条提交
                {refs.length > 0 && (
                  <span className="ml-2 font-mono text-gray-500">{refs.join(", ")}</span>
                )}
              </p>
            </div>
            <Link to="/" className="text-sm text-blue-400 hover:text-blue-300">
              返回列表
            </Link>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <label className="flex flex-col gap-1 sm:col-span-2">
              <span className="text-xs text-gray-500">目标 Git 工作区路径</span>
              <input
                type="text"
                className="rounded border border-gray-700 bg-gray-950 px-3 py-2 text-sm"
                placeholder="C:\path\to\git-repo"
                value={target.wcPath}
                onChange={(e) => setTarget({ wcPath: e.target.value })}
              />
            </label>
          </div>

          <div>
            <p className="mb-2 text-xs text-gray-500">
              将 SVN 路径映射到目标 Git 工作区内路径。若 SVN URL 已是 trunk 目录，diff 常为{" "}
              <code className="text-gray-400">/src/...</code>，请保留规则{" "}
              <code className="text-gray-400">/ → .</code>；若路径带{" "}
              <code className="text-gray-400">/trunk/</code> 前缀则用{" "}
              <code className="text-gray-400">/trunk → .</code>。
            </p>
            <div className="mb-2 flex items-center justify-between">
              <span className="text-xs font-medium text-gray-400">路径映射</span>
              <button
                type="button"
                className="text-xs text-blue-400 hover:text-blue-300"
                onClick={addMapping}
              >
                + 添加
              </button>
            </div>
            <div className="space-y-2">
              {target.mappings.map((m, i) => (
                <div key={i} className="flex gap-2">
                  <input
                    className="flex-1 rounded border border-gray-700 bg-gray-950 px-2 py-1 text-sm"
                    placeholder="/trunk"
                    value={m.from}
                    onChange={(e) => setMapping(i, { from: e.target.value })}
                  />
                  <span className="self-center text-gray-500">→</span>
                  <input
                    className="flex-1 rounded border border-gray-700 bg-gray-950 px-2 py-1 text-sm"
                    placeholder="."
                    value={m.to}
                    onChange={(e) => setMapping(i, { to: e.target.value })}
                  />
                  {target.mappings.length > 1 && (
                    <button
                      type="button"
                      className="text-xs text-red-400"
                      onClick={() => removeMapping(i)}
                    >
                      删除
                    </button>
                  )}
                </div>
              ))}
            </div>
          </div>

          <div className="flex items-center gap-3">
            <button
              type="button"
              className="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
              disabled={loading}
              onClick={() => void runPreview()}
            >
              {loading ? "生成中…" : "生成预览"}
            </button>
            {preview && (
              <span className="text-sm text-gray-400">
                {preview.stats.files_changed} 文件 · +{preview.stats.lines_added} / -
                {preview.stats.lines_removed}
                {preview.stats.conflict_risk_count > 0 && (
                  <span className="ml-2 text-orange-400">
                    {preview.stats.conflict_risk_count} 处可能冲突
                  </span>
                )}
              </span>
            )}
          </div>
          {error && <p className="text-sm text-red-400">{error}</p>}
        </div>
      </div>

      {preview && (
        <div className="flex min-h-0 flex-1">
          <aside className="flex w-64 shrink-0 flex-col border-r border-gray-800">
            <div className="flex border-b border-gray-800 text-xs">
              <button
                type="button"
                className={`flex-1 px-2 py-2 ${
                  treeMode === "aggregated" ? "bg-gray-800 text-gray-100" : "text-gray-500"
                }`}
                onClick={() => setTreeMode("aggregated")}
              >
                汇总
              </button>
              <button
                type="button"
                className={`flex-1 px-2 py-2 ${
                  treeMode === "by_unit" ? "bg-gray-800 text-gray-100" : "text-gray-500"
                }`}
                onClick={() => setTreeMode("by_unit")}
              >
                按提交
              </button>
            </div>
            <FileTree
              mode={treeMode}
              aggregated={preview.aggregated}
              units={preview.units}
              selectedPath={selectedPath}
              onSelect={setSelectedPath}
            />
          </aside>
          <main className="min-w-0 flex-1">
            <DiffViewer file={selectedFile} />
          </main>
        </div>
      )}

      {!preview && !loading && !error && (
        <p className="p-8 text-center text-sm text-gray-500">
          配置目标工作区与映射后，点击「生成预览」
        </p>
      )}
    </div>
  );
}
