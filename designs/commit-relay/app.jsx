const { useState, useMemo, useEffect, useRef } = React;

const STRATEGY_LABELS = {
  skip: "跳过",
  apply_patch: "增量补丁",
  write_after: "写入完整内容",
  manual_merge: "人工合并",
};

function PathMappingPanel({ source, target, mappings, custom, onToggle, onChange }) {
  const duplicates = mappings.filter(
    (rule, index) =>
      mappings.findIndex((candidate) => candidate.from.trim() === rule.from.trim()) !== index,
  );
  const invalid =
    mappings.length === 0 ||
    mappings.some((rule) => !rule.from.trim() || !rule.to.trim()) ||
    duplicates.length > 0;

  const update = (index, field, value) => {
    const next = [...mappings];
    next[index] = { ...next[index], [field]: value };
    onChange(next);
  };

  const move = (index, direction) => {
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= mappings.length) return;
    const next = [...mappings];
    [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
    onChange(next);
  };

  return (
    <section className="card path-mapping-panel" aria-labelledby="mapping-title">
      <div className="path-mapping-heading">
        <div>
          <h3 id="mapping-title">文件如何写入目标仓库</h3>
          <p className="form-hint">
            {custom
              ? `将源 ${source?.type === "svn" ? "SVN" : "Git"} 路径前缀映射到目标 ${target?.type === "svn" ? "SVN" : "Git"}。规则按从上到下匹配。`
              : `默认保持目录结构，写入 ${target?.name ?? "目标仓库"} 根目录。`}
          </p>
        </div>
        <label className="form-advanced-toggle">
          <input
            type="checkbox"
            checked={custom}
            onChange={(event) => onToggle(event.target.checked)}
          />
          <span>自定义对应关系</span>
        </label>
      </div>
      {custom && (
        <div className="mapping-editor">
          <div className="mapping-labels">
            <span>源路径前缀</span>
            <span>目标路径前缀</span>
            <span>顺序与操作</span>
          </div>
          {mappings.map((rule, index) => {
            const duplicate =
              mappings.findIndex((candidate) => candidate.from.trim() === rule.from.trim()) !==
              index;
            const rowInvalid = !rule.from.trim() || !rule.to.trim() || duplicate;
            return (
              <div className={`mapping-row${rowInvalid ? " invalid" : ""}`} key={index}>
                <input
                  aria-label={`第 ${index + 1} 条规则的源路径`}
                  value={rule.from}
                  placeholder={source?.type === "svn" ? "/trunk" : "."}
                  onChange={(event) => update(index, "from", event.target.value)}
                />
                <input
                  aria-label={`第 ${index + 1} 条规则的目标路径`}
                  value={rule.to}
                  placeholder={target?.type === "svn" ? "/trunk" : "."}
                  onChange={(event) => update(index, "to", event.target.value)}
                />
                <div className="mapping-actions">
                  <button
                    type="button"
                    className="icon-btn"
                    aria-label="上移规则"
                    disabled={index === 0}
                    onClick={() => move(index, -1)}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    className="icon-btn"
                    aria-label="下移规则"
                    disabled={index === mappings.length - 1}
                    onClick={() => move(index, 1)}
                  >
                    ↓
                  </button>
                  <button
                    type="button"
                    className="icon-btn danger"
                    aria-label="删除规则"
                    onClick={() => onChange(mappings.filter((_, itemIndex) => itemIndex !== index))}
                  >
                    <IconTrash />
                  </button>
                </div>
                {rowInvalid && (
                  <span className="mapping-error">
                    {duplicate ? "源路径重复" : "请填写完整路径"}
                  </span>
                )}
              </div>
            );
          })}
          <div className="mapping-footer">
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => onChange([...mappings, { from: "", to: "" }])}
            >
              <IconPlus /> 添加规则
            </button>
            <span className={`mapping-validation${invalid ? " invalid" : ""}`}>
              {invalid ? "请修正规则后继续" : `${mappings.length} 条规则有效，将按顺序匹配`}
            </span>
          </div>
          {!invalid && (
            <div className="mapping-preview" role="status">
              {mappings.map((rule, index) => (
                <code key={`${rule.from}-${index}`}>
                  {rule.from} → {rule.to}
                </code>
              ))}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function MigrationSummary({
  source,
  target,
  commitCount,
  mode,
  squash,
  message,
  mappings,
  custom,
  expanded,
  onToggle,
}) {
  const mappingLabel = custom
    ? mappings.map((rule) => `${rule.from}→${rule.to}`).join("，")
    : "保持目录结构";

  return (
    <section
      className={`card migration-summary${expanded ? " expanded" : ""}`}
      aria-labelledby="migration-summary-title"
    >
      <div className="migration-summary-compact">
        <div className="migration-summary-route">
          <span className="eyebrow">执行摘要</span>
          <strong id="migration-summary-title" title={`${source?.name} → ${target?.name}`}>
            {source?.name} <span aria-hidden="true">→</span> {target?.name}
          </strong>
        </div>
        <div className="migration-summary-facts" aria-label="迁移摘要">
          <span>{commitCount} 条提交</span>
          <span>{FILE_CHANGES.length} 个文件</span>
          <span>{MIGRATION_MODE_LABELS[mode]}</span>
          <span>{squash ? "合并提交" : "保留原提交"}</span>
        </div>
        <button
          type="button"
          className="btn btn-ghost summary-toggle"
          aria-expanded={expanded}
          aria-controls="migration-summary-details"
          onClick={onToggle}
        >
          {expanded ? "收起详情" : "展开详情"}
        </button>
      </div>
      {expanded && (
        <div id="migration-summary-details" className="migration-summary-expanded">
          <div className="migration-summary-grid">
            <div>
              <span>提交</span>
              <strong>{commitCount} 条</strong>
            </div>
            <div>
              <span>文件</span>
              <strong>{FILE_CHANGES.length} 个</strong>
            </div>
            <div>
              <span>模式</span>
              <strong>{MIGRATION_MODE_LABELS[mode]}</strong>
            </div>
            <div>
              <span>提交方式</span>
              <strong>{squash ? "合并为单次提交" : "保留原提交"}</strong>
            </div>
          </div>
          <div className="migration-summary-detail">
            <span>目标路径</span>
            <code>{target?.path}</code>
            <span>路径规则</span>
            <code>{mappingLabel}</code>
            {squash && (
              <>
                <span>提交说明</span>
                <code>{message || "尚未填写"}</code>
              </>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

function IntegrationWorkspace({
  items,
  activeId,
  acceptedIds,
  resolvedIds,
  checkingId,
  editors,
  selectedEditorId,
  target,
  collapsed,
  onToggleCollapsed,
  onSelect,
  onSelectEditor,
  onBrowseEditor,
  onOpenWithEditor,
  onAccept,
  onUndoAccept,
  onRecheck,
  onUndoResolved,
  onAcceptAll,
  focusPreview,
  onToggleFocus,
  codeZoom,
  onChangeCodeZoom,
}) {
  const [sections, setSections] = useState({ blocked: true, review: true, auto_ok: false });
  const active = items.find((item) => item.id === activeId) || items[0];
  const groups = [
    { key: "blocked", label: "需人工处理" },
    { key: "review", label: "需确认" },
    { key: "auto_ok", label: "可自动应用" },
  ];
  const statusLabel = { blocked: "需处理", review: "需确认", auto_ok: "可自动" };
  const pendingReview = items.filter(
    (item) => item.integrationStatus === "review" && !acceptedIds.has(item.id),
  ).length;

  const renderCompare = () => {
    if (!active)
      return (
        <div className="empty-state">
          <p>选择文件查看差异</p>
        </div>
      );
    const isBlocked = active.integrationStatus === "blocked";
    const isReview = active.integrationStatus === "review";
    const resolved = resolvedIds.has(active.id);
    const accepted = acceptedIds.has(active.id);
    const file = FILE_CHANGES.find((candidate) => candidate.id === active.id);
    const rows =
      active.before?.length || active.after?.length
        ? alignConflictLines(active.before || [], active.after || [])
        : [];
    return (
      <div className="integration-compare">
        <div className="conflict-compare-toolbar">
          <span className="conflict-compare-path" title={active.path}>
            {active.path}
          </span>
          <div className="conflict-compare-actions">
            {isBlocked && !resolved && (
              <EditorOpenMenu
                editors={editors}
                selectedId={selectedEditorId}
                onSelect={onSelectEditor}
                onOpen={(editorId) => onOpenWithEditor(active, editorId)}
                onAddCustom={onBrowseEditor}
              />
            )}
            {isBlocked && !resolved && (
              <button
                type="button"
                className="conflict-action-btn conflict-action-btn--resolve"
                disabled={checkingId === active.id}
                onClick={() => onRecheck(active.id)}
              >
                {checkingId === active.id ? "正在重新检测…" : "重新检测并标记解决"}
              </button>
            )}
            {isBlocked && resolved && (
              <>
                <span className="state-pill success">
                  <IconCheck /> 已重新检测
                </span>
                <button
                  type="button"
                  className="btn btn-ghost"
                  onClick={() => onUndoResolved(active.id)}
                >
                  撤销
                </button>
              </>
            )}
            {isReview && !accepted && (
              <button
                type="button"
                className="conflict-action-btn conflict-action-btn--resolve"
                onClick={() => onAccept(active.id)}
              >
                <IconCheck /> 接受写入
              </button>
            )}
            {isReview && accepted && (
              <>
                <span className="state-pill success">
                  <IconCheck /> 已接受
                </span>
                <button
                  type="button"
                  className="btn btn-ghost"
                  onClick={() => onUndoAccept(active.id)}
                >
                  撤销
                </button>
              </>
            )}
            {active.integrationStatus === "auto_ok" && (
              <span className="state-pill success">
                <IconCheck /> 可自动应用
              </span>
            )}
          </div>
        </div>
        {rows.length > 0 ? (
          <>
            <div className="conflict-compare-header">
              <span>目标仓库（当前）</span>
              <span>迁入变更（期望）</span>
            </div>
            <div className="conflict-compare-body">
              {rows.map((row, index) => (
                <div
                  key={index}
                  className={`conflict-row${row.kind === "same" ? "" : ` ${row.kind}`}`}
                >
                  <div className={`conflict-cell${row.left === null ? " empty" : ""}`}>
                    <span className="conflict-ln">{row.left !== null ? row.lineNo : ""}</span>
                    <span className="conflict-code">{row.left ?? ""}</span>
                  </div>
                  <div className={`conflict-cell${row.right === null ? " empty" : ""}`}>
                    <span className="conflict-ln">{row.right !== null ? row.lineNo : ""}</span>
                    <span className="conflict-code">{row.right ?? ""}</span>
                  </div>
                </div>
              ))}
            </div>
          </>
        ) : (
          <div className="integration-diff-wrap">
            <DiffView file={file} />
          </div>
        )}
        <div className="conflict-compare-hint">
          <span>{active.reason}</span>
          <span> · 策略：{STRATEGY_LABELS[active.strategy]}</span>
          {active.overlapLines && (
            <span>
              {" "}
              · 重叠行 {active.overlapLines[0]}–{active.overlapLines[1]}
            </span>
          )}
          {isBlocked && (
            <span>
              {" "}
              · 路径：
              <code>
                {target?.path}\{active.path.replace(/\//g, "\\")}
              </code>
            </span>
          )}
        </div>
      </div>
    );
  };

  return (
    <div
      className={`integration-workspace${collapsed ? " list-collapsed" : ""}`}
      style={{ "--diff-font-size": `${(12 * codeZoom).toFixed(1)}px` }}
    >
      <div className="integration-toolbar card">
        <div className="integration-stats">
          <span className="integration-stat integration-stat--auto_ok">
            {items.filter((item) => item.integrationStatus === "auto_ok").length} 可自动应用
          </span>
          <span className="integration-stat integration-stat--review">
            {items.filter((item) => item.integrationStatus === "review").length} 需确认
          </span>
          <span className="integration-stat integration-stat--blocked">
            {items.filter((item) => item.integrationStatus === "blocked").length} 需人工处理
          </span>
        </div>
        <div className="integration-toolbar-actions">
          <div className="code-zoom-control" role="group" aria-label="差异代码字号">
            <button
              type="button"
              aria-label="缩小代码"
              disabled={codeZoom <= 0.9}
              onClick={() => onChangeCodeZoom(Math.max(0.9, codeZoom - 0.1))}
            >
              −
            </button>
            <span>{Math.round(codeZoom * 100)}%</span>
            <button
              type="button"
              aria-label="放大代码"
              disabled={codeZoom >= 1.2}
              onClick={() => onChangeCodeZoom(Math.min(1.2, codeZoom + 0.1))}
            >
              +
            </button>
          </div>
          {pendingReview > 0 && (
            <button type="button" className="btn btn-ghost" onClick={onAcceptAll}>
              全部接受需确认项
            </button>
          )}
          <button type="button" className="btn btn-ghost pane-toggle" onClick={onToggleCollapsed}>
            {collapsed ? "显示文件列表" : "收起文件列表"}
          </button>
          <button
            type="button"
            className={`btn ${focusPreview ? "btn-primary" : "btn-ghost"}`}
            aria-pressed={focusPreview}
            onClick={onToggleFocus}
          >
            {focusPreview ? "退出专注预览" : "专注预览"}
          </button>
        </div>
      </div>
      <div className="integration-body">
        {!collapsed && (
          <div className="card integration-list" aria-label="集成计划文件列表">
            {groups.map((group) => {
              const groupItems = items.filter((item) => item.integrationStatus === group.key);
              if (groupItems.length === 0) return null;
              return (
                <section
                  className={`integration-section integration-section--${group.key}`}
                  key={group.key}
                >
                  <button
                    type="button"
                    className="integration-section-header"
                    aria-expanded={sections[group.key]}
                    onClick={() =>
                      setSections((current) => ({ ...current, [group.key]: !current[group.key] }))
                    }
                  >
                    <span>{group.label}</span>
                    <span>{groupItems.length}</span>
                  </button>
                  {sections[group.key] &&
                    groupItems.map((item) => {
                      const done =
                        item.integrationStatus === "blocked"
                          ? resolvedIds.has(item.id)
                          : item.integrationStatus === "review"
                            ? acceptedIds.has(item.id)
                            : true;
                      const parts = splitFilePath(item.path);
                      return (
                        <button
                          type="button"
                          key={item.id}
                          className={`integration-item${item.id === activeId ? " active" : ""}${done ? " done" : ""}`}
                          aria-pressed={item.id === activeId}
                          onClick={() => onSelect(item.id)}
                        >
                          <span className="integration-item-status">{done && <IconCheck />}</span>
                          <span className="integration-item-content">
                            <span className="integration-item-badges">
                              <StatusBadge status={item.status} />
                              <span
                                className={`integration-badge integration-badge--${item.integrationStatus}`}
                              >
                                {statusLabel[item.integrationStatus]}
                              </span>
                            </span>
                            <span className="integration-item-path" title={item.path}>
                              <span>{parts.dir}</span>
                              <strong>{parts.name}</strong>
                            </span>
                            <span className="integration-item-reason">{item.reason}</span>
                          </span>
                        </button>
                      );
                    })}
                </section>
              );
            })}
          </div>
        )}
        {renderCompare()}
      </div>
    </div>
  );
}

function ExecuteConfirmDialog({
  source,
  target,
  commitCount,
  mode,
  mappings,
  custom,
  onClose,
  onConfirm,
}) {
  const dialogRef = useRef(null);
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return undefined;
    const previous = document.activeElement;
    const buttons = [...dialog.querySelectorAll("button:not([disabled])")];
    buttons[0]?.focus();
    const onKeyDown = (event) => {
      if (event.key === "Escape") onClose();
      if (event.key !== "Tab" || buttons.length === 0) return;
      const first = buttons[0];
      const last = buttons[buttons.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      }
      if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previous?.focus?.();
    };
  }, [onClose]);
  return (
    <div
      className="modal-overlay"
      onMouseDown={(event) => event.target === event.currentTarget && onClose()}
    >
      <div
        ref={dialogRef}
        className="modal execute-confirm-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="execute-confirm-title"
      >
        <div className="modal-header">
          <h2 id="execute-confirm-title">确认执行迁移</h2>
          <button type="button" className="icon-btn" aria-label="关闭弹窗" onClick={onClose}>
            <IconClose />
          </button>
        </div>
        <div className="modal-body">
          <div className="confirm-route">
            <strong>{source?.name}</strong>
            <span>→</span>
            <strong>{target?.name}</strong>
          </div>
          <div className="confirm-warning">
            <IconWarn />
            <p>此操作会修改目标仓库并生成提交。开始前会创建安全检查点，失败时自动回滚。</p>
          </div>
          <dl className="confirm-list">
            <div>
              <dt>目标路径</dt>
              <dd>
                <code>{target?.path}</code>
              </dd>
            </div>
            <div>
              <dt>提交与文件</dt>
              <dd>
                {commitCount} 条提交 · {FILE_CHANGES.length} 个文件
              </dd>
            </div>
            <div>
              <dt>迁移模式</dt>
              <dd>{MIGRATION_MODE_LABELS[mode]}</dd>
            </div>
            <div>
              <dt>路径规则</dt>
              <dd>
                {custom
                  ? mappings.map((rule) => `${rule.from}→${rule.to}`).join("，")
                  : "保持目录结构"}
              </dd>
            </div>
          </dl>
        </div>
        <div className="modal-footer">
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            返回检查
          </button>
          <button type="button" className="btn btn-primary" onClick={onConfirm}>
            确认并执行
          </button>
        </div>
      </div>
    </div>
  );
}

function ExecutionProgress({ source, target, phase }) {
  const phases = [
    "创建目标仓库安全检查点",
    "应用原提交",
    "写入已确认文件",
    "生成目标提交",
    "保存迁移记录",
  ];
  const progress = [12, 36, 64, 86, 100][phase] || 12;
  return (
    <div className="main-screen" data-screen-label="迁移执行中">
      <div className="main-header">
        <div>
          <h1>正在执行迁移</h1>
          <p>
            {source?.name} → {target?.name}
          </p>
        </div>
        <span className="running-indicator">请勿关闭窗口</span>
      </div>
      <div className="main-content main-content--scroll">
        <div className="card execution-progress">
          <div className="execution-progress-head">
            <div>
              <span className="eyebrow">迁移进度</span>
              <h2>{phases[phase]}</h2>
            </div>
            <strong>{progress}%</strong>
          </div>
          <div
            className="progress-track"
            role="progressbar"
            aria-label="迁移进度"
            aria-valuenow={progress}
            aria-valuemin="0"
            aria-valuemax="100"
          >
            <span style={{ width: `${progress}%` }}></span>
          </div>
          <ol className="execution-phases">
            {phases.map((item, index) => (
              <li key={item} className={index < phase ? "done" : index === phase ? "active" : ""}>
                <span>{index < phase ? <IconCheck /> : index + 1}</span>
                <div>
                  <strong>{item}</strong>
                  <small>{index < phase ? "已完成" : index === phase ? "进行中…" : "等待中"}</small>
                </div>
              </li>
            ))}
          </ol>
          <div className="execution-current">
            <span>当前处理</span>
            <code>
              {phase < 2
                ? "a3f8c21 · fix: 修复支付回调超时重试逻辑"
                : phase === 2
                  ? "src/main/java/com/pay/CallbackHandler.java"
                  : target?.branch}
            </code>
          </div>
        </div>
      </div>
    </div>
  );
}

function App() {
  const [repos, setRepos] = useState(INITIAL_REPOS);
  const [step, setStep] = useState("source");
  const [maxReached, setMaxReached] = useState(0);
  const [showRepos, setShowRepos] = useState(false);
  const [sourceId, setSourceId] = useState(null);
  const [targetId, setTargetId] = useState(null);
  const [selectedCommits, setSelectedCommits] = useState(new Set());
  const [activeFileId, setActiveFileId] = useState("f1");
  const [previewListCollapsed, setPreviewListCollapsed] = useState(false);
  const [customMapping, setCustomMapping] = useState(false);
  const [pathMappings, setPathMappings] = useState([{ from: ".", to: "." }]);
  const [migrationMode, setMigrationMode] = useState("incremental_first");
  const [squashCommits, setSquashCommits] = useState(false);
  const [squashMessage, setSquashMessage] = useState("");
  const [activeIntegrationId, setActiveIntegrationId] = useState("f1");
  const [acceptedReviewIds, setAcceptedReviewIds] = useState(new Set());
  const [resolvedBlockedIds, setResolvedBlockedIds] = useState(new Set());
  const [checkingId, setCheckingId] = useState(null);
  const [integrationListCollapsed, setIntegrationListCollapsed] = useState(false);
  const [summaryExpanded, setSummaryExpanded] = useState(false);
  const [focusPreview, setFocusPreview] = useState(false);
  const [codeZoom, setCodeZoom] = useState(1);
  const [confirmExecute, setConfirmExecute] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [executionPhase, setExecutionPhase] = useState(0);
  const [migrated, setMigrated] = useState(false);
  const [modal, setModal] = useState(null);
  const [toast, setToast] = useState(null);
  const [settingsTab, setSettingsTab] = useState("repos");
  const [updateStatus, setUpdateStatus] = useState("available");
  const [updateProgress, setUpdateProgress] = useState(0);
  const [animateUpdate, setAnimateUpdate] = useState(false);
  const [editors, setEditors] = useState(EDITOR_PRESETS);
  const [selectedEditorId, setSelectedEditorId] = useState("vscode");
  const [migrationHistory, setMigrationHistory] = useState(INITIAL_MIGRATION_HISTORY);
  const [historyDetailId, setHistoryDetailId] = useState(null);
  const [lastMigrationId, setLastMigrationId] = useState(null);

  const source = repos.find((repo) => repo.id === sourceId);
  const target = repos.find((repo) => repo.id === targetId);
  const selectedEditor = editors.find((editor) => editor.id === selectedEditorId);
  const pendingReview = INTEGRATION_ITEMS.filter(
    (item) => item.integrationStatus === "review" && !acceptedReviewIds.has(item.id),
  );
  const pendingBlocked = INTEGRATION_ITEMS.filter(
    (item) => item.integrationStatus === "blocked" && !resolvedBlockedIds.has(item.id),
  );
  const mappingSources = pathMappings.map((rule) => rule.from.trim()).filter(Boolean);
  const mappingsValid =
    !customMapping ||
    (pathMappings.length > 0 &&
      pathMappings.every((rule) => rule.from.trim() && rule.to.trim()) &&
      new Set(mappingSources).size === mappingSources.length);
  const canExecute =
    pendingReview.length === 0 &&
    pendingBlocked.length === 0 &&
    mappingsValid &&
    (!squashCommits || squashMessage.trim());

  const completed = useMemo(() => {
    const value = new Set(STEPS.slice(0, maxReached).map((item) => item.id));
    if (migrated) value.add("migrate");
    return value;
  }, [maxReached, migrated]);

  useEffect(() => {
    if (!executing) return undefined;
    const timer = window.setTimeout(
      () => {
        if (executionPhase < 4) {
          setExecutionPhase((value) => value + 1);
          return;
        }
        const record = {
          id: `m${Date.now()}`,
          completedAt: new Date().toLocaleString("zh-CN", { hour12: false }).replace(/\//g, "-"),
          source: {
            name: source.name,
            path: source.path,
            type: source.type,
            branch: source.branch,
          },
          target: {
            name: target.name,
            path: target.path,
            type: target.type,
            branch: target.branch,
          },
          commits: COMMITS.filter((commit) => selectedCommits.has(commit.id)),
          files: FILE_CHANGES,
          conflictsResolved: resolvedBlockedIds.size,
          status: "success",
        };
        setMigrationHistory((records) => [record, ...records]);
        setLastMigrationId(record.id);
        setExecuting(false);
        setMigrated(true);
      },
      executionPhase === 4 ? 500 : 700,
    );
    return () => window.clearTimeout(timer);
  }, [executing, executionPhase]);

  useEffect(() => {
    if (updateStatus !== "downloading" || !animateUpdate) return undefined;
    const timer = window.setInterval(() => {
      setUpdateProgress((value) => {
        if (value >= 100) return value;
        const next = Math.min(100, value + 7);
        if (next === 100) {
          window.setTimeout(() => {
            setUpdateStatus("latest");
            setAnimateUpdate(false);
            setToast("更新已安装，Relay 将自动重启");
          }, 500);
        }
        return next;
      });
    }, 450);
    return () => window.clearInterval(timer);
  }, [animateUpdate, updateStatus]);

  const previewUpdateStatus = (status) => {
    setAnimateUpdate(false);
    setUpdateStatus(status);
    setUpdateProgress(status === "downloading" ? 62 : 0);
  };

  const checkForUpdates = () => {
    setAnimateUpdate(false);
    setUpdateStatus("checking");
    window.setTimeout(() => setUpdateStatus("latest"), 900);
  };

  const installUpdate = () => {
    setUpdateProgress(8);
    setAnimateUpdate(true);
    setUpdateStatus("downloading");
  };

  useEffect(() => {
    if (!focusPreview) return undefined;
    const exitFocusPreview = (event) => {
      if (event.key === "Escape") setFocusPreview(false);
    };
    document.addEventListener("keydown", exitFocusPreview);
    return () => document.removeEventListener("keydown", exitFocusPreview);
  }, [focusPreview]);

  const toggleCommit = (id) =>
    setSelectedCommits((current) => {
      const next = new Set(current);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  const selectCommits = (ids, select) =>
    setSelectedCommits((current) => {
      const next = new Set(current);
      ids.forEach((id) => (select ? next.add(id) : next.delete(id)));
      return next;
    });

  const resetPlanChecks = () => {
    setAcceptedReviewIds(new Set());
    setResolvedBlockedIds(new Set());
    setToast(<span>迁移模式已更新，集成计划已重新检测</span>);
  };

  const recheckBlocked = (id) => {
    setCheckingId(id);
    window.setTimeout(() => {
      setResolvedBlockedIds((current) => new Set([...current, id]));
      setCheckingId(null);
      setToast(<span>已重新检测目标文件，冲突处理结果有效</span>);
    }, 850);
  };

  const openInEditor = (item, editorId) => {
    const id = editorId || selectedEditorId;
    if (editorId) setSelectedEditorId(editorId);
    const editor = editors.find((candidate) => candidate.id === id) || selectedEditor;
    setToast(
      <span>
        已用 <strong>{editor?.name ?? "编辑器"}</strong> 打开{" "}
        <code>
          {target?.path}\{item.path.replace(/\//g, "\\")}
        </code>
      </span>,
    );
  };

  const saveRepo = (form) => {
    if (modal?.mode === "edit")
      setRepos((items) =>
        items.map((repo) => (repo.id === modal.repo.id ? { ...repo, ...form } : repo)),
      );
    else setRepos((items) => [...items, { ...form, id: `r${Date.now()}`, lastUsed: "刚刚" }]);
    setModal(null);
  };

  const resetMigration = () => {
    setStep("source");
    setMaxReached(0);
    setSourceId(null);
    setTargetId(null);
    setSelectedCommits(new Set());
    setAcceptedReviewIds(new Set());
    setResolvedBlockedIds(new Set());
    setCustomMapping(false);
    setPathMappings([{ from: ".", to: "." }]);
    setSummaryExpanded(false);
    setFocusPreview(false);
    setCodeZoom(1);
    setSquashCommits(false);
    setSquashMessage("");
    setMigrated(false);
    setLastMigrationId(null);
    setExecutionPhase(0);
  };

  const canNext = () => {
    if (showRepos) return false;
    if (step === "source") return !!sourceId;
    if (step === "commits") return selectedCommits.size > 0;
    if (step === "target") return !!targetId && targetId !== sourceId && mappingsValid;
    if (step === "preview") return true;
    return false;
  };

  const nextStep = () => {
    const index = STEPS.findIndex((item) => item.id === step);
    if (index < STEPS.length - 1) {
      setMaxReached((value) => Math.max(value, index + 1));
      setStep(STEPS[index + 1].id);
    }
  };
  const prevStep = () => {
    if (showRepos) {
      setShowRepos(false);
      return;
    }
    const index = STEPS.findIndex((item) => item.id === step);
    if (index > 0) {
      setFocusPreview(false);
      setStep(STEPS[index - 1].id);
    }
  };

  const renderSettings = () => (
    <div className="main-screen" data-screen-label="设置">
      <div className="main-header">
        <div>
          <h1>设置</h1>
          <p>管理仓库、编辑器、迁移记录与应用更新</p>
        </div>
      </div>
      <div className="main-content main-content--scroll">
        <div className="settings-tabs">
          {["repos", "editors", "history", "updates"].map((tab) => (
            <button
              key={tab}
              type="button"
              className={`settings-tab${settingsTab === tab ? " active" : ""}`}
              onClick={() => {
                setSettingsTab(tab);
                setHistoryDetailId(null);
              }}
            >
              {tab === "repos"
                ? "仓库"
                : tab === "editors"
                  ? "编辑器"
                  : tab === "history"
                    ? "迁移记录"
                    : "关于和更新"}
            </button>
          ))}
        </div>
        {settingsTab === "repos" ? (
          <RepoManagement
            repos={repos}
            onAdd={() => setModal({ mode: "add" })}
            onEdit={(repo) => setModal({ mode: "edit", repo })}
            onRemove={(id) => setRepos((items) => items.filter((repo) => repo.id !== id))}
          />
        ) : settingsTab === "editors" ? (
          <EditorSettings
            editors={editors}
            selectedId={selectedEditorId}
            onSelect={setSelectedEditorId}
            onAdd={() => setModal({ mode: "editor" })}
            onRemove={(id) => setEditors((items) => items.filter((editor) => editor.id !== id))}
          />
        ) : settingsTab === "updates" ? (
          <UpdateSettings
            status={updateStatus}
            progress={updateProgress}
            onCheck={checkForUpdates}
            onInstall={installUpdate}
            onLater={() => setSettingsTab("repos")}
            onPreviewStatus={previewUpdateStatus}
          />
        ) : historyDetailId ? (
          <MigrationDetail
            record={migrationHistory.find((record) => record.id === historyDetailId)}
            onBack={() => setHistoryDetailId(null)}
          />
        ) : (
          <MigrationHistory records={migrationHistory} onSelect={setHistoryDetailId} />
        )}
      </div>
    </div>
  );

  const renderContent = () => {
    if (showRepos) return renderSettings();
    if (step === "source")
      return (
        <div className="main-screen" data-screen-label="源仓库">
          <div className="main-header">
            <div>
              <h1>选择源仓库</h1>
              <p>从已保存的仓库中选择提交来源，或添加新仓库</p>
            </div>
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => setModal({ mode: "add" })}
            >
              <IconPlus /> 添加仓库
            </button>
          </div>
          <div className="main-content main-content--scroll">
            <div className="repo-grid">
              {repos.map((repo) => (
                <RepoCard
                  key={repo.id}
                  repo={repo}
                  selected={sourceId === repo.id}
                  onClick={() => setSourceId(repo.id)}
                />
              ))}
            </div>
          </div>
        </div>
      );
    if (step === "commits")
      return (
        <div className="main-screen" data-screen-label="选择提交">
          <div className="main-header">
            <div>
              <h1>选择提交</h1>
              <p>
                来自 <strong>{source?.name}</strong> · 最近 {COMMITS.length} 条提交，已选{" "}
                {selectedCommits.size} 条
              </p>
            </div>
          </div>
          <div className="main-content main-content--commits">
            <CommitPicker
              commits={COMMITS}
              pageSize={COMMIT_PAGE_SIZE}
              selectedIds={selectedCommits}
              onToggle={toggleCommit}
              onSelectMany={selectCommits}
            />
          </div>
        </div>
      );
    if (step === "target") {
      const available = repos.filter((repo) => repo.id !== sourceId);
      return (
        <div className="main-screen" data-screen-label="目标仓库">
          <div className="main-header">
            <div>
              <h1>选择目标仓库</h1>
              <p>确认迁移目标，并检查文件写入规则</p>
            </div>
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => setModal({ mode: "add" })}
            >
              <IconPlus /> 添加仓库
            </button>
          </div>
          <div className="main-content main-content--scroll">
            <div className="migrate-flow">
              <div className="migrate-node">
                <div className="migrate-node-label">源</div>
                <div className="migrate-node-name">{source?.name}</div>
                <div className="migrate-node-path">{source?.path}</div>
                <div style={{ marginTop: 8 }}>
                  <VcsBadge type={source?.type} />
                </div>
              </div>
              <div className="migrate-connector">
                <IconArrow />
              </div>
              <div
                className="migrate-node"
                style={{ borderStyle: targetId ? "solid" : "dashed", opacity: targetId ? 1 : 0.65 }}
              >
                <div className="migrate-node-label">目标</div>
                {target ? (
                  <>
                    <div className="migrate-node-name">{target.name}</div>
                    <div className="migrate-node-path">{target.path}</div>
                    <div style={{ marginTop: 8 }}>
                      <VcsBadge type={target.type} />
                    </div>
                  </>
                ) : (
                  <div className="migrate-node-placeholder">请选择下方目标仓库</div>
                )}
              </div>
            </div>
            <div className="repo-grid">
              {available.map((repo) => (
                <RepoCard
                  key={repo.id}
                  repo={repo}
                  selected={targetId === repo.id}
                  onClick={() => setTargetId(repo.id)}
                />
              ))}
            </div>
            {targetId && (
              <PathMappingPanel
                source={source}
                target={target}
                mappings={pathMappings}
                custom={customMapping}
                onToggle={(checked) => {
                  setCustomMapping(checked);
                  if (checked && pathMappings.length === 0)
                    setPathMappings([{ from: ".", to: "." }]);
                }}
                onChange={setPathMappings}
              />
            )}
          </div>
        </div>
      );
    }
    if (step === "preview") {
      const file = FILE_CHANGES.find((candidate) => candidate.id === activeFileId);
      const stats = migrationStats(FILE_CHANGES);
      return (
        <div className="main-screen main-screen--code" data-screen-label="变更预览">
          <div className="main-header">
            <div>
              <h1>变更预览</h1>
              <p>{selectedCommits.size} 条提交 · 合并后净变更</p>
            </div>
            <button
              type="button"
              className="btn btn-ghost pane-toggle"
              onClick={() => setPreviewListCollapsed((value) => !value)}
            >
              {previewListCollapsed ? "显示文件列表" : "收起文件列表"}
            </button>
          </div>
          <div className="main-content">
            <div className="summary-bar">
              <div className="summary-stat">
                <span className="badge badge-add">新增</span>
                <strong>{stats.adds}</strong> 个文件
              </div>
              <div className="summary-stat">
                <span className="badge badge-mod">修改</span>
                <strong>{stats.mods}</strong> 个文件
              </div>
              <div className="summary-stat">
                <span className="badge badge-del">删除</span>
                <strong>{stats.dels}</strong> 个文件
              </div>
              <div className="summary-stat summary-lines">
                +{stats.additions} / −{stats.deletions} 行
              </div>
            </div>
            <div className={`preview-layout${previewListCollapsed ? " list-collapsed" : ""}`}>
              {!previewListCollapsed && (
                <FileTree files={FILE_CHANGES} activeId={activeFileId} onSelect={setActiveFileId} />
              )}
              <DiffView file={file} />
            </div>
          </div>
        </div>
      );
    }
    if (step === "migrate") {
      if (migrated)
        return (
          <div className="main-screen" data-screen-label="迁移完成">
            <div className="main-content main-content--scroll">
              <div className="success-panel">
                <div className="success-icon">
                  <IconSuccess />
                </div>
                <h2>迁移完成</h2>
                <p>
                  已将 {selectedCommits.size} 条提交从 <strong>{source?.name}</strong> 成功应用到{" "}
                  <strong>{target?.name}</strong>。共变更 {FILE_CHANGES.length} 个文件。
                </p>
                <div className="success-actions">
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => {
                      setShowRepos(true);
                      setSettingsTab("history");
                      setHistoryDetailId(lastMigrationId);
                    }}
                  >
                    查看本次记录
                  </button>
                  <button type="button" className="btn btn-primary" onClick={resetMigration}>
                    开始新的迁移
                  </button>
                </div>
              </div>
            </div>
          </div>
        );
      if (executing)
        return <ExecutionProgress source={source} target={target} phase={executionPhase} />;
      return (
        <div className="main-screen main-screen--code" data-screen-label="迁移确认">
          <div className="main-header">
            <div>
              <h1>确认迁移</h1>
              <p>审查目标、模式与集成计划，确认后执行</p>
            </div>
          </div>
          <div className="main-content main-content--commits">
            <MigrationSummary
              source={source}
              target={target}
              commitCount={selectedCommits.size}
              mode={migrationMode}
              squash={squashCommits}
              message={squashMessage}
              mappings={pathMappings}
              custom={customMapping}
              expanded={summaryExpanded}
              onToggle={() => setSummaryExpanded((value) => !value)}
            />
            <div className="card migration-controls">
              <label htmlFor="migration-mode">迁移模式</label>
              <select
                id="migration-mode"
                value={migrationMode}
                onChange={(event) => {
                  setMigrationMode(event.target.value);
                  resetPlanChecks();
                }}
              >
                {Object.entries(MIGRATION_MODE_LABELS).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
              <label className="integration-squash-toggle">
                <input
                  type="checkbox"
                  checked={squashCommits}
                  onChange={(event) => setSquashCommits(event.target.checked)}
                />
                合并为单次提交
              </label>
              {squashCommits && (
                <input
                  className="integration-squash-message"
                  aria-label="合并后的提交说明"
                  placeholder="填写提交说明"
                  value={squashMessage}
                  onChange={(event) => setSquashMessage(event.target.value)}
                />
              )}
            </div>
            <IntegrationWorkspace
              items={INTEGRATION_ITEMS}
              activeId={activeIntegrationId}
              acceptedIds={acceptedReviewIds}
              resolvedIds={resolvedBlockedIds}
              checkingId={checkingId}
              editors={editors}
              selectedEditorId={selectedEditorId}
              target={target}
              collapsed={integrationListCollapsed}
              onToggleCollapsed={() => setIntegrationListCollapsed((value) => !value)}
              onSelect={setActiveIntegrationId}
              onSelectEditor={setSelectedEditorId}
              onBrowseEditor={() => setModal({ mode: "editor" })}
              onOpenWithEditor={openInEditor}
              onAccept={(id) => setAcceptedReviewIds((current) => new Set([...current, id]))}
              onUndoAccept={(id) =>
                setAcceptedReviewIds((current) => {
                  const next = new Set(current);
                  next.delete(id);
                  return next;
                })
              }
              onRecheck={recheckBlocked}
              onUndoResolved={(id) =>
                setResolvedBlockedIds((current) => {
                  const next = new Set(current);
                  next.delete(id);
                  return next;
                })
              }
              onAcceptAll={() =>
                setAcceptedReviewIds(
                  new Set(
                    INTEGRATION_ITEMS.filter((item) => item.integrationStatus === "review").map(
                      (item) => item.id,
                    ),
                  ),
                )
              }
              focusPreview={focusPreview}
              onToggleFocus={() => {
                setSummaryExpanded(false);
                setFocusPreview((value) => !value);
              }}
              codeZoom={codeZoom}
              onChangeCodeZoom={(value) => setCodeZoom(Number(value.toFixed(1)))}
            />
          </div>
        </div>
      );
    }
    return null;
  };

  const footerHint = () => {
    if (showRepos)
      return settingsTab === "repos"
        ? `${repos.length} 个已保存仓库`
        : settingsTab === "editors"
          ? `默认编辑器：${selectedEditor?.name ?? "未选择"}`
          : settingsTab === "updates"
            ? "当前版本 v0.3.0"
            : `${migrationHistory.length} 条迁移记录`;
    if (step === "source" && !sourceId) return "请选择一个源仓库";
    if (step === "commits") return `已选择 ${selectedCommits.size} 条提交`;
    if (step === "target" && !targetId) return "请选择目标仓库";
    if (step === "target" && !mappingsValid) return "请修正路径映射规则";
    if (step === "preview") return `${FILE_CHANGES.length} 个文件待迁移`;
    if (step === "migrate" && !canExecute) {
      const parts = [];
      if (pendingReview.length) parts.push(`${pendingReview.length} 个待确认`);
      if (pendingBlocked.length) parts.push(`${pendingBlocked.length} 个待处理`);
      if (squashCommits && !squashMessage.trim()) parts.push("缺少提交说明");
      return parts.join(" · ");
    }
    return step === "migrate" ? "集成计划已就绪" : "";
  };

  return (
    <div className="app-shell">
      <TitleBar
        updateVersion={
          updateStatus === "available" || updateStatus === "downloading" ? "0.4.0" : null
        }
        onOpenUpdate={() => {
          setShowRepos(true);
          setSettingsTab("updates");
          setHistoryDetailId(null);
        }}
      />
      <div className="body">
        <StepRail
          steps={STEPS}
          current={showRepos ? null : step}
          completed={completed}
          maxReached={maxReached}
          onStep={(id) => {
            setShowRepos(false);
            if (id !== "migrate") setFocusPreview(false);
            setStep(id);
          }}
          onManageRepos={() => {
            setShowRepos(true);
            setSettingsTab("repos");
            setHistoryDetailId(null);
          }}
          showRepos={showRepos}
        />
        <div
          className={`main-area${
            !showRepos && step === "migrate" && !executing && !migrated && focusPreview
              ? " focus-preview"
              : ""
          }`}
        >
          {renderContent()}
          {(!migrated || showRepos) && (
            <div className="main-footer">
              <span className="footer-left">{footerHint()}</span>
              <div className="footer-actions">
                {!showRepos && step !== "source" && !executing && (
                  <button type="button" className="btn btn-ghost" onClick={prevStep}>
                    上一步
                  </button>
                )}
                {showRepos ? (
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={() => setShowRepos(false)}
                  >
                    返回迁移
                  </button>
                ) : step === "migrate" ? (
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={!canExecute || executing}
                    onClick={() => setConfirmExecute(true)}
                  >
                    {executing
                      ? `正在迁移 ${[12, 36, 64, 86, 100][executionPhase]}%`
                      : "核对并执行"}
                  </button>
                ) : (
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={!canNext()}
                    onClick={nextStep}
                  >
                    下一步 <IconChevronRight />
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {modal?.mode === "editor" ? (
        <EditorAppModal
          onClose={() => setModal(null)}
          onSave={({ name, exe }) => {
            const id = `custom-${Date.now()}`;
            setEditors((items) => [...items, { id, name, exe, kind: "custom", custom: true }]);
            setSelectedEditorId(id);
            setModal(null);
          }}
        />
      ) : modal ? (
        <RepoModal
          repo={modal.mode === "edit" ? modal.repo : null}
          onClose={() => setModal(null)}
          onSave={saveRepo}
        />
      ) : null}
      {confirmExecute && (
        <ExecuteConfirmDialog
          source={source}
          target={target}
          commitCount={selectedCommits.size}
          mode={migrationMode}
          mappings={pathMappings}
          custom={customMapping}
          onClose={() => setConfirmExecute(false)}
          onConfirm={() => {
            setConfirmExecute(false);
            setFocusPreview(false);
            setExecutionPhase(0);
            setExecuting(true);
          }}
        />
      )}
      {toast && <Toast message={toast} onDone={() => setToast(null)} />}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
