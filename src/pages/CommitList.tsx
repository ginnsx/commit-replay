import { useCallback, useState } from "react";
import { Link } from "react-router-dom";
import { listCommits } from "../lib/invoke";
import type { ReplayUnitMeta } from "../lib/types";
import { useConnectionStore } from "../store/connectionStore";
import { useSelectionStore } from "../store/selection";

function formatDate(iso: string): string {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

function revisionFromRef(sourceRef: string): string {
  return sourceRef.replace(/^svn:/, "");
}

export default function CommitList() {
  const { svn, setSvn } = useConnectionStore();
  const { selectedRefs, toggle, selectAll, clear, isSelected } = useSelectionStore();
  const [commits, setCommits] = useState<ReplayUnitMeta[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchCommits = useCallback(async () => {
    if (!svn.url.trim()) {
      setError("请填写 SVN 仓库 URL");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const list = await listCommits(
        "svn",
        svn.url.trim(),
        svn.limit,
        svn.username || undefined,
        svn.password || undefined,
      );
      setCommits(list);
      clear();
    } catch (e) {
      setError(String(e));
      setCommits([]);
    } finally {
      setLoading(false);
    }
  }, [svn, clear]);

  const toggleAll = () => {
    if (selectedRefs.size === commits.length && commits.length > 0) {
      clear();
    } else {
      selectAll(commits.map((c) => c.source_ref));
    }
  };

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-6 p-6">
      <header>
        <h1 className="text-2xl font-bold text-gray-100">提交列表</h1>
        <p className="mt-1 text-sm text-gray-400">
          从 SVN 源仓库拉取最近提交，勾选后进入预览（步骤三）。
        </p>
      </header>

      <section className="rounded-lg border border-gray-800 bg-gray-900/60 p-4">
        <h2 className="mb-3 text-sm font-medium text-gray-300">SVN 连接</h2>
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="flex flex-col gap-1 sm:col-span-2">
            <span className="text-xs text-gray-500">仓库 URL</span>
            <input
              type="url"
              className="rounded border border-gray-700 bg-gray-950 px-3 py-2 text-sm text-gray-100"
              placeholder="https://svn.example.com/repo/trunk"
              value={svn.url}
              onChange={(e) => setSvn({ url: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-xs text-gray-500">用户名</span>
            <input
              type="text"
              className="rounded border border-gray-700 bg-gray-950 px-3 py-2 text-sm text-gray-100"
              value={svn.username}
              onChange={(e) => setSvn({ username: e.target.value })}
              autoComplete="username"
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-xs text-gray-500">密码</span>
            <input
              type="password"
              className="rounded border border-gray-700 bg-gray-950 px-3 py-2 text-sm text-gray-100"
              value={svn.password}
              onChange={(e) => setSvn({ password: e.target.value })}
              autoComplete="current-password"
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-xs text-gray-500">条数</span>
            <input
              type="number"
              min={1}
              max={200}
              className="rounded border border-gray-700 bg-gray-950 px-3 py-2 text-sm text-gray-100"
              value={svn.limit}
              onChange={(e) => setSvn({ limit: Number(e.target.value) || 20 })}
            />
          </label>
        </div>
        <div className="mt-4 flex items-center gap-3">
          <button
            type="button"
            className="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
            onClick={() => void fetchCommits()}
            disabled={loading}
          >
            {loading ? "加载中…" : "拉取提交"}
          </button>
          {selectedRefs.size > 0 && (
            <>
              <span className="text-sm text-gray-400">已选 {selectedRefs.size} 条</span>
              <Link
                to="/preview"
                className="rounded border border-gray-600 px-4 py-2 text-sm text-gray-200 hover:border-gray-500 hover:bg-gray-800"
              >
                进入预览
              </Link>
            </>
          )}
        </div>
        {error && <p className="mt-3 text-sm text-red-400">{error}</p>}
      </section>

      {commits.length > 0 && (
        <section className="overflow-hidden rounded-lg border border-gray-800">
          <table className="w-full text-left text-sm">
            <thead className="bg-gray-900 text-gray-400">
              <tr>
                <th className="w-10 px-3 py-2">
                  <input
                    type="checkbox"
                    checked={selectedRefs.size === commits.length && commits.length > 0}
                    onChange={toggleAll}
                    aria-label="全选"
                  />
                </th>
                <th className="px-3 py-2">Revision</th>
                <th className="px-3 py-2">作者</th>
                <th className="px-3 py-2">时间</th>
                <th className="px-3 py-2">文件数</th>
                <th className="px-3 py-2">说明</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-800 bg-gray-950/50">
              {commits.map((c) => (
                <tr key={c.source_ref} className="hover:bg-gray-900/80">
                  <td className="px-3 py-2">
                    <input
                      type="checkbox"
                      checked={isSelected(c.source_ref)}
                      onChange={() => toggle(c.source_ref)}
                      aria-label={`选择 ${c.source_ref}`}
                    />
                  </td>
                  <td className="px-3 py-2 font-mono text-blue-300">
                    r{revisionFromRef(c.source_ref)}
                  </td>
                  <td className="px-3 py-2 text-gray-300">{c.author || "—"}</td>
                  <td className="whitespace-nowrap px-3 py-2 text-gray-400">
                    {formatDate(c.date)}
                  </td>
                  <td className="px-3 py-2 text-gray-400">{c.changed_paths_count}</td>
                  <td className="max-w-md truncate px-3 py-2 text-gray-300" title={c.message}>
                    {c.message || "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {!loading && commits.length === 0 && !error && (
        <p className="text-center text-sm text-gray-500">填写 URL 后点击「拉取提交」</p>
      )}
    </div>
  );
}
