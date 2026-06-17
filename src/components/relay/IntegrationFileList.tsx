import { useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { IntegrationItemView } from "../../lib/types";
import { splitFilePath } from "../../lib/constants";
import { StatusBadge } from "./Badges";
import { IconCheck, IconChevronDown } from "./icons";

const ROW_GAP = 4;
const HEADER_HEIGHT = 38;
const FILE_ROW_ESTIMATE = 80;

type SectionKey = "blocked" | "review" | "auto_ok";

type ListEntry =
  | {
      kind: "header";
      key: string;
      section: SectionKey;
      title: string;
      count: number;
      collapsed: boolean;
    }
  | { kind: "file"; key: string; item: IntegrationItemView };

const SECTIONS: { key: SectionKey; title: string; defaultCollapsed: boolean }[] = [
  { key: "blocked", title: "需人工处理", defaultCollapsed: false },
  { key: "review", title: "需确认", defaultCollapsed: false },
  { key: "auto_ok", title: "可自动应用", defaultCollapsed: true },
];

function statusLabel(status: IntegrationItemView["integrationStatus"]) {
  if (status === "review") return "需确认";
  if (status === "blocked") return "需处理";
  return "可自动";
}

function IntegrationFileRow({
  item,
  activeId,
  acceptedReviewIds,
  resolvedBlockedIds,
  onSelect,
}: {
  item: IntegrationItemView;
  activeId: string | null;
  acceptedReviewIds: Set<string>;
  resolvedBlockedIds: Set<string>;
  onSelect: (id: string) => void;
}) {
  const { dir, name } = splitFilePath(item.path);
  const isReview = item.integrationStatus === "review";
  const done = isReview
    ? acceptedReviewIds.has(item.id)
    : item.integrationStatus === "blocked"
      ? resolvedBlockedIds.has(item.id)
      : false;

  return (
    <button
      type="button"
      className={`conflict-item${item.id === activeId ? " active" : ""}${done ? " resolved" : ""}`}
      onClick={() => onSelect(item.id)}
    >
      <div className="conflict-item-status">{done && <IconCheck />}</div>
      <div className="conflict-item-body">
        <div className="conflict-item-badges">
          <StatusBadge status={item.status} />
          <span className={`integration-badge integration-badge--${item.integrationStatus}`}>
            {statusLabel(item.integrationStatus)}
          </span>
        </div>
        <div className="conflict-item-path" title={item.path}>
          {dir ? <span className="conflict-item-dir">{dir}</span> : null}
          <span className="conflict-item-name">{name}</span>
        </div>
        <div className="conflict-item-reason" title={item.reason}>
          {item.reason}
        </div>
      </div>
    </button>
  );
}

export function IntegrationFileList({
  items,
  activeId,
  acceptedReviewIds,
  resolvedBlockedIds,
  onSelect,
}: {
  items: IntegrationItemView[];
  activeId: string | null;
  acceptedReviewIds: Set<string>;
  resolvedBlockedIds: Set<string>;
  onSelect: (id: string) => void;
}) {
  const listRef = useRef<HTMLDivElement>(null);
  const [collapsed, setCollapsed] = useState<Record<SectionKey, boolean>>(() =>
    Object.fromEntries(SECTIONS.map((s) => [s.key, s.defaultCollapsed])) as Record<SectionKey, boolean>,
  );

  const grouped = useMemo(
    () => ({
      blocked: items.filter((i) => i.integrationStatus === "blocked"),
      review: items.filter((i) => i.integrationStatus === "review"),
      auto_ok: items.filter((i) => i.integrationStatus === "auto_ok"),
    }),
    [items],
  );

  const entries = useMemo(() => {
    const result: ListEntry[] = [];
    for (const section of SECTIONS) {
      const group = grouped[section.key];
      if (group.length === 0) continue;
      const isCollapsed = collapsed[section.key];
      result.push({
        kind: "header",
        key: `header-${section.key}`,
        section: section.key,
        title: section.title,
        count: group.length,
        collapsed: isCollapsed,
      });
      if (!isCollapsed) {
        for (const item of group) {
          result.push({ kind: "file", key: item.id, item });
        }
      }
    }
    return result;
  }, [collapsed, grouped]);

  const virtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => listRef.current,
    estimateSize: (index) => (entries[index]?.kind === "header" ? HEADER_HEIGHT : FILE_ROW_ESTIMATE),
    gap: ROW_GAP,
    overscan: 8,
  });

  const toggleSection = (section: SectionKey) => {
    setCollapsed((prev) => ({ ...prev, [section]: !prev[section] }));
  };

  return (
    <div className="card integration-file-list">
      <div ref={listRef} className="integration-file-list-scroll">
        <div
          className="integration-file-list-inner"
          style={{ height: virtualizer.getTotalSize(), position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((vItem) => {
            const entry = entries[vItem.index];
            if (!entry) return null;

            return (
              <div
                key={entry.key}
                data-index={vItem.index}
                ref={virtualizer.measureElement}
                className="integration-file-list-item"
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${vItem.start}px)`,
                }}
              >
                {entry.kind === "header" ? (
                  <div className={`integration-section integration-section--${entry.section}`}>
                    <button
                      type="button"
                      className="integration-section-header"
                      onClick={() => toggleSection(entry.section)}
                      aria-expanded={!entry.collapsed}
                    >
                      <span className={`integration-section-chevron${entry.collapsed ? "" : " open"}`}>
                        <IconChevronDown />
                      </span>
                      <span className="integration-section-title">{entry.title}</span>
                      <span className="integration-section-count">{entry.count}</span>
                    </button>
                  </div>
                ) : (
                  <IntegrationFileRow
                    item={entry.item}
                    activeId={activeId}
                    acceptedReviewIds={acceptedReviewIds}
                    resolvedBlockedIds={resolvedBlockedIds}
                    onSelect={onSelect}
                  />
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
