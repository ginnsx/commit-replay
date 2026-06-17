import { useEffect, useRef, useState } from "react";
import type { CommitListItem } from "../../lib/types";
import { CommitRow } from "./CommitRow";
import { IconClose, IconSearch } from "./icons";

export function CommitPicker({
  commits,
  pageSize,
  selectedIds,
  onToggle,
  onSelectMany,
  hasRemoteMore,
  onFetchRemote,
  fetchingRemote,
}: {
  commits: CommitListItem[];
  pageSize: number;
  selectedIds: Set<string>;
  onToggle: (id: string) => void;
  onSelectMany: (ids: string[], select: boolean) => void;
  hasRemoteMore?: boolean;
  onFetchRemote?: () => void | Promise<void>;
  fetchingRemote?: boolean;
}) {
  const [search, setSearch] = useState("");
  const [visibleCount, setVisibleCount] = useState(pageSize);
  const [loadingMore, setLoadingMore] = useState(false);
  const pendingExpandRef = useRef(false);
  const prevCommitCountRef = useRef(commits.length);
  const q = search.trim().toLowerCase();
  const filtered = commits.filter((c) => {
    if (!q) return true;
    return (
      c.hash.toLowerCase().includes(q) ||
      c.msg.toLowerCase().includes(q) ||
      c.author.toLowerCase().includes(q) ||
      c.date.includes(q)
    );
  });
  const visible = filtered.slice(0, visibleCount);
  const hasMoreLocal = visibleCount < filtered.length;
  const allFilteredSelected = filtered.length > 0 && filtered.every((c) => selectedIds.has(c.id));
  const showLoadMore = hasMoreLocal || !!hasRemoteMore;
  const loading = loadingMore || !!fetchingRemote;

  useEffect(() => {
    setVisibleCount(pageSize);
  }, [q, pageSize]);

  useEffect(() => {
    if (pendingExpandRef.current && commits.length > prevCommitCountRef.current) {
      pendingExpandRef.current = false;
      setVisibleCount((n) => Math.min(n + pageSize, filtered.length));
    }
    prevCommitCountRef.current = commits.length;
  }, [commits.length, filtered.length, pageSize]);

  const toggleSelectFiltered = () => {
    if (allFilteredSelected) {
      onSelectMany(
        filtered.map((c) => c.id),
        false,
      );
    } else {
      onSelectMany(
        filtered.map((c) => c.id),
        true,
      );
    }
  };

  const loadMore = async () => {
    if (loading) return;
    if (hasMoreLocal) {
      setVisibleCount((n) => Math.min(n + pageSize, filtered.length));
      return;
    }
    if (hasRemoteMore && onFetchRemote) {
      setLoadingMore(true);
      pendingExpandRef.current = true;
      try {
        await onFetchRemote();
      } finally {
        setLoadingMore(false);
      }
    }
  };

  return (
    <div className="card commit-picker">
      <div className="commit-picker-toolbar">
        <div className="commit-search">
          <IconSearch />
          <input
            className="commit-search-input"
            placeholder="搜索 hash、说明、作者…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          {search && (
            <button
              type="button"
              className="commit-search-clear"
              aria-label="清除搜索"
              onClick={() => setSearch("")}
            >
              <IconClose />
            </button>
          )}
        </div>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={filtered.length === 0}
          onClick={toggleSelectFiltered}
        >
          {allFilteredSelected ? "取消全选" : "全选"}
        </button>
      </div>
      {filtered.length === 0 ? (
        <div className="commit-picker-empty">
          <p>
            {fetchingRemote && commits.length === 0
              ? "加载中…"
              : commits.length === 0
                ? "暂无提交"
                : "没有匹配的提交"}
          </p>
          {search && (
            <button type="button" className="btn btn-ghost" style={{ fontSize: 12 }} onClick={() => setSearch("")}>
              清除搜索
            </button>
          )}
        </div>
      ) : (
        <>
          <div className="commit-list">
            {visible.map((c) => (
              <CommitRow
                key={c.id}
                commit={c}
                selected={selectedIds.has(c.id)}
                onToggle={() => onToggle(c.id)}
              />
            ))}
          </div>
          <div className="commit-picker-footer">
            <span className="commit-picker-count">
              已显示 {visible.length} / {filtered.length} 条
              {q ? `（共 ${commits.length} 条）` : ""}
            </span>
            {showLoadMore && (
              <button
                type="button"
                className={`btn btn-ghost commit-load-more${loading ? " loading" : ""}`}
                disabled={loading}
                onClick={loadMore}
              >
                {loading
                  ? "加载中…"
                  : `加载更多（${Math.min(pageSize, hasMoreLocal ? filtered.length - visible.length : pageSize)} 条）`}
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}
