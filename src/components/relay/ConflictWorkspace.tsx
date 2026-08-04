import { useEffect, useRef, useState, type CSSProperties } from "react";
import type {
  DiffLine,
  Editor,
  IntegrationItemView,
  MigrationMode,
  PathMapping,
  Repo,
} from "../../lib/types";
import { targetFilePath } from "../../lib/constants";
import { IconCheck, IconChevronDown } from "./icons";
import { IntegrationFileList } from "./IntegrationFileList";
import { MigrationSummary } from "./MigrationSummary";

const MODE_LABELS: Record<MigrationMode, string> = {
  incremental_first: "增量优先",
  commit_result: "以提交结果为准",
  strict_replay: "严格 replay",
};

const STRATEGY_LABELS: Record<IntegrationItemView["strategy"], string> = {
  skip: "跳过",
  apply_patch: "增量补丁",
  write_after: "写入完整内容",
  manual_merge: "人工合并",
};

function diffCompareRows(diff: DiffLine[]) {
  return diff.map((line, index) => ({
    key: index,
    leftLn: line.old ?? "",
    rightLn: line.new ?? "",
    left: line.type === "ctx" || line.type === "del" ? line.text : null,
    right: line.type === "ctx" || line.type === "add" ? line.text : null,
    kind: line.type === "ctx" ? "same" : line.type,
  }));
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
          <button
            type="button"
            className="editor-open-item editor-open-item-add"
            onClick={onAddCustom}
          >
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
  const rows = diffCompareRows(item.diff ?? []);
  const fullPath = target ? targetFilePath(target.path, item.path) : item.path;
  const leftPaneRef = useRef<HTMLDivElement>(null);
  const rightPaneRef = useRef<HTMLDivElement>(null);
  const syncingScroll = useRef(false);
  const isReview = item.integrationStatus === "review";
  const isBlocked = item.integrationStatus === "blocked";
  const done = isReview ? accepted : isBlocked ? resolved : true;

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
          {item.integrationStatus !== "auto_ok" && (
            <EditorOpenMenu
              editors={editors}
              selectedId={selectedEditorId}
              onSelect={onSelectEditor}
              onOpen={() => onOpenWithEditor()}
              onAddCustom={onBrowseEditor}
            />
          )}
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
              <div key={row.key} className={`conflict-row ${row.kind}`}>
                <span className="conflict-ln">{row.leftLn}</span>
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
              <div key={row.key} className={`conflict-row ${row.kind}`}>
                <span className="conflict-ln">{row.rightLn}</span>
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
        <span> · 策略：{STRATEGY_LABELS[item.strategy]}</span>
        {item.overlapLines && (
          <span>
            {" "}
            · 重叠行 {item.overlapLines[0]}–{item.overlapLines[1]}
          </span>
        )}
        {done && item.integrationStatus !== "auto_ok" && <span> · 已确认</span>}
        {target && item.integrationStatus !== "auto_ok" && (
          <span>
            {" "}
            · 路径：<code>{fullPath}</code>
          </span>
        )}
      </div>
    </div>
  );
}

export function ConflictWorkspace({
  items,
  source,
  target,
  commitCount,
  fileCount,
  mappings,
  customMapping,
  migrationMode,
  onMigrationModeChange,
  squashCommits,
  onSquashCommitsChange,
  squashCommitMessage,
  onSquashCommitMessageChange,
  autoOkCount,
  reviewCount,
  blockedCount,
  activeId,
  acceptedReviewIds,
  resolvedBlockedIds,
  editors,
  selectedEditorId,
  onSelectEditor,
  onBrowseEditor,
  onSelect,
  onOpenWithEditor,
  onAcceptReview,
  onMarkResolved,
  onAcceptAllReview,
  canExecute,
  summaryExpanded,
  onToggleSummary,
  focusPreview,
  onToggleFocusPreview,
  codeZoom,
  onCodeZoomChange,
}: {
  items: IntegrationItemView[];
  source?: Repo;
  target?: Repo;
  commitCount: number;
  fileCount: number;
  mappings: PathMapping[];
  customMapping: boolean;
  migrationMode: MigrationMode;
  onMigrationModeChange: (mode: MigrationMode) => void;
  squashCommits: boolean;
  onSquashCommitsChange: (value: boolean) => void;
  squashCommitMessage: string;
  onSquashCommitMessageChange: (value: string) => void;
  autoOkCount: number;
  reviewCount: number;
  blockedCount: number;
  activeId: string | null;
  acceptedReviewIds: Set<string>;
  resolvedBlockedIds: Set<string>;
  editors: Editor[];
  selectedEditorId: string;
  onSelectEditor: (id: string) => void;
  onBrowseEditor: () => void;
  onSelect: (id: string) => void;
  onOpenWithEditor: (item: IntegrationItemView, editorId?: string) => void;
  onAcceptReview: (id: string) => void;
  onMarkResolved: (id: string) => void;
  onAcceptAllReview: () => void;
  canExecute: boolean;
  summaryExpanded: boolean;
  onToggleSummary: () => void;
  focusPreview: boolean;
  onToggleFocusPreview: () => void;
  codeZoom: number;
  onCodeZoomChange: (value: number) => void;
}) {
  const active = items.find((c) => c.id === activeId);
  const pendingReview = items.filter(
    (i) => i.integrationStatus === "review" && !acceptedReviewIds.has(i.id),
  ).length;
  const pendingBlocked = items.filter(
    (i) => i.integrationStatus === "blocked" && !resolvedBlockedIds.has(i.id),
  ).length;

  return (
    <div
      className="integration-workspace"
      style={{ "--diff-font-size": `${(12 * codeZoom).toFixed(1)}px` } as CSSProperties}
    >
      <MigrationSummary
        source={source}
        target={target}
        commitCount={commitCount}
        fileCount={fileCount}
        migrationMode={migrationMode}
        squashCommits={squashCommits}
        mappings={mappings}
        customMapping={customMapping}
        expanded={summaryExpanded}
        onToggle={onToggleSummary}
      />
      <div className="migration-controls card">
        <label htmlFor="migration-mode">迁移模式</label>
        <select
          id="migration-mode"
          value={migrationMode}
          onChange={(event) => onMigrationModeChange(event.target.value as MigrationMode)}
        >
          {(Object.keys(MODE_LABELS) as MigrationMode[]).map((mode) => (
            <option key={mode} value={mode}>
              {MODE_LABELS[mode]}
            </option>
          ))}
        </select>
        <label className="integration-squash-toggle">
          <input
            type="checkbox"
            checked={squashCommits}
            onChange={(event) => onSquashCommitsChange(event.target.checked)}
          />
          合并为单次提交
        </label>
        {squashCommits && (
          <input
            type="text"
            className="integration-squash-message"
            aria-label="合并后的提交说明"
            placeholder="填写提交说明"
            value={squashCommitMessage}
            onChange={(event) => onSquashCommitMessageChange(event.target.value)}
          />
        )}
      </div>
      <div className="integration-toolbar card">
        <div className="integration-summary-stats">
          <span className="integration-stat integration-stat--auto_ok">
            {autoOkCount} 可自动应用
          </span>
          <span className="integration-stat integration-stat--review">{reviewCount} 需确认</span>
          <span className="integration-stat integration-stat--blocked">
            {blockedCount} 需人工处理
          </span>
          <span className={`integration-ready${canExecute ? " ready" : ""}`} role="status">
            {canExecute ? (
              <>
                <IconCheck /> 已就绪
              </>
            ) : (
              <>
                {pendingReview > 0 && `${pendingReview} 个待确认`}
                {pendingReview > 0 && pendingBlocked > 0 && " · "}
                {pendingBlocked > 0 && `${pendingBlocked} 个待处理`}
              </>
            )}
          </span>
        </div>
        <div className="integration-toolbar-actions">
          <div className="code-zoom-control" role="group" aria-label="差异代码字号">
            <button
              type="button"
              aria-label="缩小代码"
              disabled={codeZoom <= 0.9}
              onClick={() => onCodeZoomChange(Math.max(0.9, codeZoom - 0.1))}
            >
              −
            </button>
            <span>{Math.round(codeZoom * 100)}%</span>
            <button
              type="button"
              aria-label="放大代码"
              disabled={codeZoom >= 1.2}
              onClick={() => onCodeZoomChange(Math.min(1.2, codeZoom + 0.1))}
            >
              +
            </button>
          </div>
          {pendingReview > 0 && (
            <button type="button" className="btn btn-ghost" onClick={onAcceptAllReview}>
              全部接受需确认项
            </button>
          )}
          <button
            type="button"
            className={`btn ${focusPreview ? "btn-primary" : "btn-ghost"}`}
            aria-pressed={focusPreview}
            onClick={onToggleFocusPreview}
          >
            {focusPreview ? "退出专注预览" : "专注预览"}
          </button>
        </div>
      </div>
      <div className="conflict-workspace">
        <IntegrationFileList
          items={items}
          activeId={activeId}
          acceptedReviewIds={acceptedReviewIds}
          resolvedBlockedIds={resolvedBlockedIds}
          onSelect={onSelect}
        />
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
