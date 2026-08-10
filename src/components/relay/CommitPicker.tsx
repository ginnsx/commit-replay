import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { CommitListItem } from "../../lib/types";
import { CommitRow } from "./CommitRow";
import { IconClose, IconSearch } from "./icons";

const ROW_GAP = 4;
const ROW_ESTIMATE = 68;

export function CommitPicker({
  commits,
  selectedIds,
  relayedIds,
  onToggle,
  onSelectMany,
  hasRemoteMore,
  onFetchRemote,
  fetchingRemote,
}: {
  commits: CommitListItem[];
  selectedIds: Set<string>;
  relayedIds?: Set<string>;
  onToggle: (id: string) => void;
  onSelectMany: (ids: string[], select: boolean) => void;
  hasRemoteMore?: boolean;
  onFetchRemote?: () => void | Promise<void>;
  fetchingRemote?: boolean;
}) {
  const [search, setSearch] = useState("");
  const [loadingMore, setLoadingMore] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  const q = search.trim().toLowerCase();

  const filtered = useMemo(() => {
    if (!q) return commits;
    return commits.filter(
      (c) =>
        c.hash.toLowerCase().includes(q) ||
        c.msg.toLowerCase().includes(q) ||
        c.author.toLowerCase().includes(q) ||
        c.date.includes(q),
    );
  }, [commits, q]);

  const allFilteredSelected = useMemo(() => {
    if (filtered.length === 0 || selectedIds.size < filtered.length) return false;
    return filtered.every((c) => selectedIds.has(c.id));
  }, [filtered, selectedIds]);

  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => listRef.current,
    estimateSize: () => ROW_ESTIMATE,
    gap: ROW_GAP,
    overscan: 8,
  });

  const loading = loadingMore || !!fetchingRemote;

  const loadMore = useCallback(async () => {
    if (loading || !hasRemoteMore || !onFetchRemote) return;
    setLoadingMore(true);
    try {
      await onFetchRemote();
    } finally {
      setLoadingMore(false);
    }
  }, [hasRemoteMore, loading, onFetchRemote]);

  useEffect(() => {
    const el = listRef.current;
    if (!el) return undefined;
    const onScroll = () => {
      if (loading || !hasRemoteMore) return;
      if (el.scrollTop + el.clientHeight >= el.scrollHeight - ROW_ESTIMATE * 4) {
        void loadMore();
      }
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [hasRemoteMore, loadMore, loading]);

  const toggleSelectFiltered = () => {
    onSelectMany(
      filtered.map((c) => c.id),
      !allFilteredSelected,
    );
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
            <button
              type="button"
              className="btn btn-ghost"
              style={{ fontSize: 12 }}
              onClick={() => setSearch("")}
            >
              清除搜索
            </button>
          )}
        </div>
      ) : (
        <>
          <div ref={listRef} className="commit-list-scroll">
            <div
              className="commit-list"
              style={{ height: virtualizer.getTotalSize(), position: "relative" }}
            >
              {virtualizer.getVirtualItems().map((item) => {
                const commit = filtered[item.index];
                return (
                  <div
                    key={commit.id}
                    data-index={item.index}
                    ref={virtualizer.measureElement}
                    className="commit-list-item"
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      transform: `translateY(${item.start}px)`,
                    }}
                  >
                    <CommitRow
                      commit={commit}
                      selected={selectedIds.has(commit.id)}
                      relayed={relayedIds?.has(commit.id)}
                      onToggle={onToggle}
                    />
                  </div>
                );
              })}
            </div>
          </div>
          <div className="commit-picker-footer">
            <span className="commit-picker-count">
              共 {filtered.length} 条{q ? `（全部 ${commits.length} 条）` : ""}
              {selectedIds.size > 0 ? ` · 已选 ${selectedIds.size} 条` : ""}
              {relayedIds && relayedIds.size > 0 ? ` · ${relayedIds.size} 条已提交` : ""}
            </span>
            {hasRemoteMore && (
              <button
                type="button"
                className={`btn btn-ghost commit-load-more${loading ? " loading" : ""}`}
                disabled={loading}
                onClick={() => void loadMore()}
              >
                {loading ? "加载中…" : "加载更早提交"}
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}
