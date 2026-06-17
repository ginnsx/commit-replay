import { useEffect, useRef, useState } from "react";
import type { Editor, IntegrationItemView, MigrationMode, Repo } from "../../lib/types";
import { splitFilePath, targetFilePath } from "../../lib/constants";
import { StatusBadge } from "./Badges";
import { IconCheck, IconChevronDown } from "./icons";

const MODE_LABELS: Record<MigrationMode, string> = {
  incremental_first: "增量优先",
  commit_result: "以提交结果为准",
  strict_replay: "严格 replay",
};

function alignRows(before: string[], after: string[]) {
  const max = Math.max(before.length, after.length);
  const rows: { left: string | null; right: string | null; kind: string; lineNo: number }[] = [];
  for (let i = 0; i < max; i++) {
    const left = before[i] ?? null;
    const right = after[i] ?? null;
    const kind =
      left === right ? "same" : left === null ? "add" : right === null ? "del" : "chg";
    rows.push({ left, right, kind, lineNo: i + 1 });
  }
  return rows;
}

function EditorOpenMenu({
  editors,
  selectedId,
  onSelect,
  onOpen,
  onAddCustom,
}: {
  editors: Editor[];
  selectedId: string;
  onSelect: (id: string) => void;
  onOpen: () => void;
  onAddCustom: () => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const selected = editors.find((e) => e.id === selectedId) ?? editors[0];

  useEffect(() => {
    if (!open) return undefined;
    const close = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  return (
    <div className="editor-open-menu" ref={ref}>
      <div className={`editor-open-split${open ? " open" : ""}`}>
        <button type="button" className="editor-open-main" onClick={onOpen}>
          <span className="editor-open-label">用 {selected?.name ?? "编辑器"} 打开</span>
        </button>
        <button
          type="button"
          className="editor-open-chevron"
          aria-label="选择编辑器"
          onClick={() => setOpen((v) => !v)}
        >
          <IconChevronDown />
        </button>
      </div>
      {open && (
        <div className="editor-open-dropdown">
          {editors.map((e) => (
            <button
              key={e.id}
              type="button"
              className={`editor-open-item${e.id === selectedId ? " active" : ""}`}
              onClick={() => {
                onSelect(e.id);
                setOpen(false);
              }}
            >
              {e.name}
            </button>
          ))}
          <button type="button" className="editor-open-item editor-open-item-add" onClick={onAddCustom}>
            选择其他应用…
          </button>
        </div>
      )}
    </div>
  );
}

function IntegrationCompareView({
  item,
  target,
  editors,
  selectedEditorId,
  onSelectEditor,
  onBrowseEditor,
  onOpenWithEditor,
  onAcceptReview,
  onMarkResolved,
  accepted,
  resolved,
}: {
  item: IntegrationItemView;
  target?: Repo;
  editors: Editor[];
  selectedEditorId: string;
  onSelectEditor: (id: string) => void;
  onBrowseEditor: () => void;
  onOpenWithEditor: (editorId?: string) => void;
  onAcceptReview: () => void;
  onMarkResolved: () => void;
  accepted: boolean;
  resolved: boolean;
}) {
  const rows = alignRows(item.before, item.after);
  const fullPath = target ? targetFilePath(target.path, item.path) : item.path;
  const leftPaneRef = useRef<HTMLDivElement>(null);
  const rightPaneRef = useRef<HTMLDivElement>(null);
  const syncingScroll = useRef(false);
  const isReview = item.integrationStatus === "review";
  const isBlocked = item.integrationStatus === "blocked";
  const done = isReview ? accepted : resolved;

  const syncVerticalScroll = (source: "left" | "right") => {
    if (syncingScroll.current) return;
    const from = source === "left" ? leftPaneRef.current : rightPaneRef.current;
    const to = source === "left" ? rightPaneRef.current : leftPaneRef.current;
    if (!from || !to) return;
    syncingScroll.current = true;
    to.scrollTop = from.scrollTop;
    syncingScroll.current = false;
  };

  return (
    <div className="conflict-compare">
      <div className="conflict-compare-toolbar">
        <span className="conflict-compare-path">{item.path}</span>
        <div className="conflict-compare-actions">
          <EditorOpenMenu
            editors={editors}
            selectedId={selectedEditorId}
            onSelect={onSelectEditor}
            onOpen={() => onOpenWithEditor()}
            onAddCustom={onBrowseEditor}
          />
          {isReview && !accepted && (
            <button
              type="button"
              className="conflict-action-btn conflict-action-btn--resolve"
              onClick={onAcceptReview}
            >
              <IconCheck /> 接受写入
            </button>
          )}
          {isBlocked && !resolved && (
            <button
              type="button"
              className="conflict-action-btn conflict-action-btn--resolve"
              onClick={onMarkResolved}
            >
              <IconCheck /> 标记已解决
            </button>
          )}
        </div>
      </div>
      <div className="conflict-compare-header">
        <span>目标仓库（当前）</span>
        <span>迁入变更（期望）</span>
      </div>
      <div className="conflict-compare-body">
        <div
          className="conflict-compare-pane"
          ref={leftPaneRef}
          onScroll={() => syncVerticalScroll("left")}
        >
          <div className="conflict-compare-lines">
            {rows.map((row) => (
              <div key={row.lineNo} className={`conflict-row ${row.kind}`}>
                <span className="conflict-ln">{row.lineNo}</span>
                <span className={`conflict-code${row.left === null ? " empty" : ""}`}>
                  {row.left ?? ""}
                </span>
              </div>
            ))}
          </div>
        </div>
        <div
          className="conflict-compare-pane"
          ref={rightPaneRef}
          onScroll={() => syncVerticalScroll("right")}
        >
          <div className="conflict-compare-lines">
            {rows.map((row) => (
              <div key={row.lineNo} className={`conflict-row ${row.kind}`}>
                <span className="conflict-ln">{row.lineNo}</span>
                <span className={`conflict-code${row.right === null ? " empty" : ""}`}>
                  {row.right ?? ""}
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
      <div className="conflict-compare-hint">
        <span>{item.reason}</span>
        {item.overlapLines && (
          <span>
            {" "}
            · 重叠行 {item.overlapLines[0]}–{item.overlapLines[1]}
          </span>
        )}
        {done && <span> · 已确认</span>}
        {target && (
          <span>
            {" "}
            · 路径：<code>{fullPath}</code>
          </span>
        )}
      </div>
    </div>
  );
}

function statusLabel(status: IntegrationItemView["integrationStatus"]) {
  if (status === "review") return "需确认";
  if (status === "blocked") return "需处理";
  return "可自动";
}

export function ConflictWorkspace({
  items,
  migrationMode,
  onMigrationModeChange,
  autoOkCount,
  activeId,
  acceptedReviewIds,
  resolvedBlockedIds,
  target,
  editors,
  selectedEditorId,
  onSelectEditor,
  onBrowseEditor,
  onSelect,
  onOpenWithEditor,
  onAcceptReview,
  onMarkResolved,
  onAcceptAllReview,
}: {
  items: IntegrationItemView[];
  migrationMode: MigrationMode;
  onMigrationModeChange: (mode: MigrationMode) => void;
  autoOkCount: number;
  activeId: string | null;
  acceptedReviewIds: Set<string>;
  resolvedBlockedIds: Set<string>;
  target?: Repo;
  editors: Editor[];
  selectedEditorId: string;
  onSelectEditor: (id: string) => void;
  onBrowseEditor: () => void;
  onSelect: (id: string) => void;
  onOpenWithEditor: (item: IntegrationItemView, editorId?: string) => void;
  onAcceptReview: (id: string) => void;
  onMarkResolved: (id: string) => void;
  onAcceptAllReview: () => void;
}) {
  const active = items.find((c) => c.id === activeId);
  const reviewItems = items.filter((i) => i.integrationStatus === "review");
  const blockedItems = items.filter((i) => i.integrationStatus === "blocked");
  const pendingReview = reviewItems.filter((i) => !acceptedReviewIds.has(i.id)).length;
  const pendingBlocked = blockedItems.filter((i) => !resolvedBlockedIds.has(i.id)).length;

  return (
    <div className="integration-workspace">
      <div className="integration-summary card">
        <div className="integration-summary-stats">
          <span>{autoOkCount} 个可自动应用</span>
          <span>{reviewItems.length} 个需确认</span>
          <span>{blockedItems.length} 个需人工处理</span>
        </div>
        <div className="integration-summary-mode">
          <label htmlFor="migration-mode">迁移模式</label>
          <select
            id="migration-mode"
            value={migrationMode}
            onChange={(e) => onMigrationModeChange(e.target.value as MigrationMode)}
          >
            {(Object.keys(MODE_LABELS) as MigrationMode[]).map((mode) => (
              <option key={mode} value={mode}>
                {MODE_LABELS[mode]}
              </option>
            ))}
          </select>
          {pendingReview > 0 && (
            <button type="button" className="btn btn-ghost" onClick={onAcceptAllReview}>
              全部接受需确认项
            </button>
          )}
        </div>
      </div>
      <div className="conflict-workspace">
        <div className="conflict-list card" style={{ padding: 12, display: "flex", flexDirection: "column" }}>
          {items.map((cf) => {
            const { dir, name } = splitFilePath(cf.path);
            const isReview = cf.integrationStatus === "review";
            const done = isReview
              ? acceptedReviewIds.has(cf.id)
              : resolvedBlockedIds.has(cf.id);
            return (
              <button
                key={cf.id}
                type="button"
                className={`conflict-item${cf.id === activeId ? " active" : ""}${done ? " resolved" : ""}`}
                onClick={() => onSelect(cf.id)}
              >
                <div className="conflict-item-status">{done && <IconCheck />}</div>
                <div className="conflict-item-body">
                  <div className="conflict-item-title">
                    <StatusBadge status={cf.status} />
                    <span className={`integration-badge integration-badge--${cf.integrationStatus}`}>
                      {statusLabel(cf.integrationStatus)}
                    </span>
                    <div className="conflict-item-path" title={cf.path}>
                      <span className="conflict-item-name">{name}</span>
                      <span className="conflict-item-dir">{dir}</span>
                    </div>
                  </div>
                  <div className="conflict-item-reason" title={cf.reason}>
                    {cf.reason}
                  </div>
                </div>
              </button>
            );
          })}
          <div style={{ marginTop: "auto", paddingTop: 8, fontSize: 11, color: "var(--text-muted)" }}>
            {pendingReview > 0 && `还有 ${pendingReview} 个待确认`}
            {pendingReview > 0 && pendingBlocked > 0 && " · "}
            {pendingBlocked > 0 && `还有 ${pendingBlocked} 个待处理`}
            {pendingReview === 0 && pendingBlocked === 0 && "全部已确认"}
          </div>
        </div>
        {active ? (
          <IntegrationCompareView
            item={active}
            target={target}
            editors={editors}
            selectedEditorId={selectedEditorId}
            onSelectEditor={onSelectEditor}
            onBrowseEditor={onBrowseEditor}
            onOpenWithEditor={(editorId) => onOpenWithEditor(active, editorId)}
            onAcceptReview={() => onAcceptReview(active.id)}
            onMarkResolved={() => onMarkResolved(active.id)}
            accepted={acceptedReviewIds.has(active.id)}
            resolved={resolvedBlockedIds.has(active.id)}
          />
        ) : (
          <div className="conflict-compare">
            <div className="empty-state">
              <p>选择左侧文件查看差异</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
