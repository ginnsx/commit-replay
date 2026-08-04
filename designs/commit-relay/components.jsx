const { useState, useEffect, useRef } = React;

function useDialogBehavior(dialogRef, onClose) {
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return undefined;
    const previous = document.activeElement;
    const focusable = () => [
      ...dialog.querySelectorAll(
        'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    ];
    focusable()[0]?.focus();
    const onKeyDown = (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key !== "Tab") return;
      const items = focusable();
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previous?.focus?.();
    };
  }, [dialogRef, onClose]);
}

function VcsBadge({ type }) {
  const isGit = type === "git";
  return (
    <span className={`badge ${isGit ? "badge-git" : "badge-svn"}`}>
      {isGit ? <IconGit /> : <IconSvn />}
      {isGit ? "Git" : "SVN"}
    </span>
  );
}

function StatusBadge({ status }) {
  const cls = { add: "badge-add", mod: "badge-mod", del: "badge-del" }[status];
  return <span className={`badge ${cls}`}>{STATUS_LABELS[status]}</span>;
}

function TitleBar() {
  return (
    <div className="titlebar">
      <div className="titlebar-drag">
        <div className="app-logo">R</div>
        <div className="app-title">
          <strong>Relay</strong> — 跨仓库提交迁移
        </div>
      </div>
      <div className="win-controls">
        <button className="win-btn" aria-label="最小化">
          <IconMin />
        </button>
        <button className="win-btn" aria-label="最大化">
          <IconMax />
        </button>
        <button className="win-btn close" aria-label="关闭">
          <IconClose />
        </button>
      </div>
    </div>
  );
}

function StepRail({ steps, current, completed, maxReached, onStep, onManageRepos, showRepos }) {
  return (
    <nav className="step-rail">
      <div className="step-rail-header">迁移流程</div>
      <div className="step-list">
        {steps.map((step, i) => {
          const active = step.id === current;
          const done = completed.has(step.id) && !active;
          const reachable = i <= maxReached;
          return (
            <button
              key={step.id}
              className={`step-item${active ? " active" : ""}${done ? " done" : ""}`}
              disabled={!reachable && !showRepos}
              onClick={() => reachable && onStep(step.id)}
            >
              <span className="step-num">{done ? <IconCheck /> : step.num}</span>
              <span className="step-label">{step.label}</span>
            </button>
          );
        })}
      </div>
      <div className="rail-footer">
        <button className={`rail-footer-btn${showRepos ? " active" : ""}`} onClick={onManageRepos}>
          <IconSettings />
          设置
        </button>
      </div>
    </nav>
  );
}

function RepoCard({ repo, selected, onClick }) {
  return (
    <button className={`repo-card${selected ? " selected" : ""}`} onClick={onClick}>
      <div className="repo-card-top">
        <div>
          <div className="repo-card-name">{repo.name}</div>
          <div className="repo-card-path">{repo.path}</div>
        </div>
        <VcsBadge type={repo.type} />
      </div>
      <div className="repo-card-meta">
        <span>{repo.branch}</span>
        <span>·</span>
        <span>最近使用 {repo.lastUsed}</span>
      </div>
    </button>
  );
}

function CommitPicker({ commits, pageSize, selectedIds, onToggle, onSelectMany }) {
  const [search, setSearch] = useState("");
  const [visibleCount, setVisibleCount] = useState(pageSize);
  const [loadingMore, setLoadingMore] = useState(false);
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
  const hasMore = visibleCount < filtered.length;
  const allFilteredSelected = filtered.length > 0 && filtered.every((c) => selectedIds.has(c.id));

  React.useEffect(() => {
    setVisibleCount(pageSize);
  }, [q, pageSize]);

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

  const loadMore = () => {
    if (!hasMore || loadingMore) return;
    setLoadingMore(true);
    window.setTimeout(() => {
      setVisibleCount((n) => Math.min(n + pageSize, filtered.length));
      setLoadingMore(false);
    }, 420);
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
          <p>没有匹配的提交</p>
          <button
            type="button"
            className="btn btn-ghost"
            style={{ fontSize: 12 }}
            onClick={() => setSearch("")}
          >
            清除搜索
          </button>
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
              已显示 {visible.length} / {filtered.length} 条{q ? `（共 ${commits.length} 条）` : ""}
            </span>
            {hasMore && (
              <button
                type="button"
                className={`btn btn-ghost commit-load-more${loadingMore ? " loading" : ""}`}
                disabled={loadingMore}
                onClick={loadMore}
              >
                {loadingMore
                  ? "加载中…"
                  : `加载更多（${Math.min(pageSize, filtered.length - visible.length)} 条）`}
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function CommitRow({ commit, selected, onToggle }) {
  return (
    <button
      type="button"
      className={`commit-row${selected ? " selected" : ""}`}
      aria-pressed={selected}
      onClick={onToggle}
    >
      <div className="commit-check">{selected && <IconCheck />}</div>
      <span className="commit-hash">{commit.hash}</span>
      <span className="commit-msg">{commit.msg}</span>
      <div className="commit-meta">
        <span>{commit.author}</span>
        <span>{commit.date}</span>
        <span>{commit.files} 个文件</span>
      </div>
    </button>
  );
}

function FileTree({ files, activeId, onSelect }) {
  const counts = { add: 0, mod: 0, del: 0 };
  files.forEach((f) => counts[f.status]++);
  return (
    <div className="file-tree">
      <div className="file-tree-header">
        <span>文件变更</span>
        <span>
          <span className="badge badge-add">+{counts.add}</span>{" "}
          <span className="badge badge-mod">~{counts.mod}</span>{" "}
          <span className="badge badge-del">−{counts.del}</span>
        </span>
      </div>
      {files.map((f) => (
        <button
          type="button"
          key={f.id}
          className={`file-item${activeId === f.id ? " active" : ""}`}
          aria-pressed={activeId === f.id}
          onClick={() => onSelect(f.id)}
        >
          <StatusBadge status={f.status} />
          <span className="file-item-path" title={f.path}>
            <span className="file-item-name">{splitFilePath(f.path).name}</span>
            <span className="file-item-dir">{splitFilePath(f.path).dir}</span>
          </span>
        </button>
      ))}
    </div>
  );
}

function DiffView({ file }) {
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
        {file.diff.map((line, i) => (
          <div key={i} className={`diff-line ${line.type}`}>
            <span className={`diff-ln${line.old ? " old" : ""}`}>{line.old ?? ""}</span>
            <span className={`diff-ln${line.new ? " new" : ""}`}>{line.new ?? ""}</span>
            <span className="diff-code">
              {line.type === "add" ? "+ " : line.type === "del" ? "- " : "  "}
              {line.text}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function EditorOpenMenu({ editors, selectedId, onSelect, onOpen, onAddCustom, stopPropagation }) {
  const { useState, useEffect, useRef } = React;
  const [open, setOpen] = useState(false);
  const ref = useRef(null);
  const selected = editors.find((e) => e.id === selectedId) || editors[0];

  useEffect(() => {
    if (!open) return undefined;
    const close = (e) => {
      if (ref.current && !ref.current.contains(e.target)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const selectEditor = (id, e) => {
    if (stopPropagation) e.stopPropagation();
    onSelect(id);
    setOpen(false);
  };

  const openFile = (e) => {
    if (stopPropagation) e.stopPropagation();
    if (selectedId || selected?.id) onOpen(selectedId || selected.id);
  };

  const toggleMenu = (e) => {
    if (stopPropagation) e.stopPropagation();
    setOpen((v) => !v);
  };

  return (
    <div className="editor-open-menu" ref={ref}>
      <div className={`editor-open-split${open ? " open" : ""}`}>
        <button
          type="button"
          className="editor-open-main"
          title={selected ? `用 ${selected.name} 打开` : "用编辑器打开"}
          onClick={openFile}
        >
          <span className="editor-open-label">用 {selected?.name ?? "编辑器"} 打开</span>
        </button>
        <button
          type="button"
          className="editor-open-chevron"
          title="选择编辑器"
          aria-label="选择编辑器"
          aria-expanded={open}
          onClick={toggleMenu}
        >
          <IconChevronDown />
        </button>
      </div>
      {open && (
        <div className="editor-open-dropdown">
          {editors.map((ed) => (
            <button
              key={ed.id}
              type="button"
              className={`editor-open-item${ed.id === selectedId ? " active" : ""}`}
              onClick={(e) => selectEditor(ed.id, e)}
            >
              {ed.name}
            </button>
          ))}
          <button
            type="button"
            className="editor-open-item editor-open-item-add"
            onClick={(e) => {
              if (stopPropagation) e.stopPropagation();
              setOpen(false);
              onAddCustom();
            }}
          >
            选择其他应用…
          </button>
        </div>
      )}
    </div>
  );
}

function EditorAppModal({ onClose, onSave }) {
  const [name, setName] = useState("");
  const [exe, setExe] = useState("");
  const dialogRef = useRef(null);
  useDialogBehavior(dialogRef, onClose);

  const mockBrowse = () => {
    setExe("D:\\Tools\\MyEditor\\editor.exe");
    if (!name) setName("MyEditor");
  };

  return (
    <div className="modal-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={dialogRef}
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="editor-modal-title"
      >
        <div className="modal-header">
          <h2 id="editor-modal-title">选择编辑器应用</h2>
          <button type="button" className="icon-btn" aria-label="关闭弹窗" onClick={onClose}>
            <IconClose />
          </button>
        </div>
        <div className="modal-body">
          <p className="form-hint" style={{ marginBottom: 4 }}>
            选择用于打开冲突文件的 .exe 程序，将加入编辑器列表。
          </p>
          <div className="form-group">
            <label htmlFor="editor-name">显示名称</label>
            <input
              id="editor-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如 Vim"
            />
          </div>
          <div className="form-group">
            <label htmlFor="editor-path">应用程序路径</label>
            <div style={{ display: "flex", gap: 8 }}>
              <input
                id="editor-path"
                style={{ flex: 1 }}
                value={exe}
                onChange={(e) => setExe(e.target.value)}
                placeholder="C:\...\editor.exe"
              />
              <button className="btn btn-ghost" style={{ flexShrink: 0 }} onClick={mockBrowse}>
                <IconFolder /> 浏览
              </button>
            </div>
          </div>
        </div>
        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose}>
            取消
          </button>
          <button
            className="btn btn-primary"
            disabled={!name || !exe}
            onClick={() => onSave({ name, exe })}
          >
            添加
          </button>
        </div>
      </div>
    </div>
  );
}

function EditorSettings({ editors, selectedId, onSelect, onAdd, onRemove }) {
  return (
    <div className="editor-settings" data-screen-label="编辑器设置">
      <p className="editor-settings-intro">
        选择默认用于打开冲突文件的外部编辑器。也可添加自定义应用程序。
      </p>
      <div className="editor-option-list">
        {editors.map((ed) => (
          <div key={ed.id} className={`editor-option${selectedId === ed.id ? " selected" : ""}`}>
            <button
              type="button"
              className="editor-option-main"
              aria-pressed={selectedId === ed.id}
              onClick={() => onSelect(ed.id)}
            >
              <span className="editor-option-radio"></span>
              <span className="editor-option-info">
                <span className="editor-option-name">{ed.name}</span>
                <span className="editor-option-exe">{ed.exe}</span>
              </span>
            </button>
            {ed.custom && (
              <button
                type="button"
                className="icon-btn danger"
                aria-label={`移除 ${ed.name}`}
                onClick={() => onRemove(ed.id)}
              >
                <IconTrash />
              </button>
            )}
          </div>
        ))}
      </div>
      <button className="btn btn-ghost" onClick={onAdd}>
        <IconPlus /> 添加编辑器应用
      </button>
    </div>
  );
}

function Toast({ message, onDone }) {
  React.useEffect(() => {
    const t = setTimeout(onDone, 3200);
    return () => clearTimeout(t);
  }, [onDone]);
  return (
    <div className="toast" role="status" aria-live="polite">
      {message}
    </div>
  );
}

function RepoModal({ repo, onClose, onSave }) {
  const isEdit = !!repo;
  const [form, setForm] = useState(
    repo || {
      name: "",
      path: "",
      type: "git",
      branch: "main",
      svnUser: "",
      svnPass: "",
    },
  );
  const set = (k, v) => setForm((f) => ({ ...f, [k]: v }));
  const dialogRef = useRef(null);
  useDialogBehavior(dialogRef, onClose);
  const modalTitleId = isEdit ? "edit-repo-title" : "add-repo-title";

  return (
    <div className="modal-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={dialogRef}
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={modalTitleId}
      >
        <div className="modal-header">
          <h2 id={modalTitleId}>{isEdit ? "编辑仓库" : "添加仓库"}</h2>
          <button type="button" className="icon-btn" aria-label="关闭弹窗" onClick={onClose}>
            <IconClose />
          </button>
        </div>
        <div className="modal-body">
          <div className="form-group">
            <label htmlFor="repo-name">显示名称</label>
            <input
              id="repo-name"
              value={form.name}
              onChange={(e) => set("name", e.target.value)}
              placeholder="例如 payment-service"
            />
          </div>
          <div className="form-group">
            <label htmlFor="repo-path">本地路径</label>
            <div style={{ display: "flex", gap: 8 }}>
              <input
                id="repo-path"
                style={{ flex: 1 }}
                value={form.path}
                onChange={(e) => set("path", e.target.value)}
                placeholder="D:\Projects\my-repo"
              />
              <button className="btn btn-ghost" style={{ flexShrink: 0 }}>
                <IconFolder /> 浏览
              </button>
            </div>
          </div>
          <div className="form-row">
            <div className="form-group">
              <label htmlFor="repo-type">版本控制</label>
              <select
                id="repo-type"
                value={form.type}
                onChange={(e) => set("type", e.target.value)}
              >
                <option value="git">Git</option>
                <option value="svn">SVN</option>
              </select>
            </div>
            <div className="form-group">
              <label htmlFor="repo-branch">{form.type === "git" ? "分支" : "路径"}</label>
              <input
                id="repo-branch"
                value={form.branch}
                onChange={(e) => set("branch", e.target.value)}
                placeholder={form.type === "git" ? "main" : "trunk"}
              />
            </div>
          </div>
          {form.type === "svn" && (
            <>
              <div className="form-row">
                <div className="form-group">
                  <label htmlFor="svn-user">SVN 用户名</label>
                  <input
                    id="svn-user"
                    value={form.svnUser}
                    onChange={(e) => set("svnUser", e.target.value)}
                  />
                </div>
                <div className="form-group">
                  <label htmlFor="svn-pass">密码</label>
                  <input
                    id="svn-pass"
                    type="password"
                    value={form.svnPass}
                    onChange={(e) => set("svnPass", e.target.value)}
                    placeholder="••••••••"
                  />
                </div>
              </div>
              <p className="form-hint">凭据将加密保存在本地，避免每次重复输入</p>
            </>
          )}
        </div>
        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose}>
            取消
          </button>
          <button
            className="btn btn-primary"
            disabled={!form.name.trim() || !form.path.trim()}
            onClick={() => onSave(form)}
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}

function CommitRowReadonly({ commit }) {
  return (
    <div className="commit-row readonly">
      <div className="commit-check">
        <IconCheck />
      </div>
      <span className="commit-hash">{commit.hash}</span>
      <span className="commit-msg">{commit.msg}</span>
      <div className="commit-meta">
        <span>{commit.author}</span>
        <span>{commit.date}</span>
        <span>{commit.files} 个文件</span>
      </div>
    </div>
  );
}

function MigrationHistory({ records, onSelect }) {
  const [search, setSearch] = useState("");
  const filtered = records.filter((r) => {
    const q = search.toLowerCase();
    return (
      r.source.name.toLowerCase().includes(q) ||
      r.target.name.toLowerCase().includes(q) ||
      r.completedAt.includes(q)
    );
  });

  return (
    <div className="migration-history" data-screen-label="迁移记录">
      <div className="repo-mgmt-toolbar">
        <input
          className="search-input"
          placeholder="搜索源/目标仓库、时间…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <span className="history-count">{records.length} 条记录</span>
      </div>
      {filtered.length === 0 ? (
        <div className="empty-state card" style={{ padding: 40 }}>
          <p>{records.length === 0 ? "暂无迁移记录" : "没有匹配的记录"}</p>
        </div>
      ) : (
        <div className="card migration-history-list">
          {filtered.map((record) => {
            const stats = migrationStats(record.files);
            return (
              <button
                key={record.id}
                type="button"
                className="history-item"
                onClick={() => onSelect(record.id)}
              >
                <div className="history-item-main">
                  <div className="history-item-time">{record.completedAt}</div>
                  <div className="history-item-flow">
                    <span className="history-repo">{record.source.name}</span>
                    <VcsBadge type={record.source.type} />
                    <span className="history-arrow">
                      <IconArrow />
                    </span>
                    <span className="history-repo">{record.target.name}</span>
                    <VcsBadge type={record.target.type} />
                  </div>
                  <div className="history-item-meta">
                    <span>{record.commits.length} 条提交</span>
                    <span>·</span>
                    <span>{record.files.length} 个文件</span>
                    <span>·</span>
                    <span className="badge badge-add">+{stats.adds}</span>
                    <span className="badge badge-mod">~{stats.mods}</span>
                    <span className="badge badge-del">−{stats.dels}</span>
                    {record.conflictsResolved > 0 && (
                      <>
                        <span>·</span>
                        <span>解决 {record.conflictsResolved} 处冲突</span>
                      </>
                    )}
                  </div>
                </div>
                <span className="history-item-chevron">
                  <IconChevronRight />
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function MigrationDetail({ record, onBack }) {
  const [detailTab, setDetailTab] = useState("commits");
  const [activeFileId, setActiveFileId] = useState(record?.files[0]?.id ?? null);

  if (!record) {
    return (
      <div className="empty-state card" style={{ padding: 40 }}>
        <p>记录不存在</p>
        <button type="button" className="btn btn-ghost" onClick={onBack}>
          返回列表
        </button>
      </div>
    );
  }
  const stats = migrationStats(record.files);
  const activeFile = record.files.find((f) => f.id === activeFileId);

  return (
    <div className="migration-detail" data-screen-label="迁移记录明细">
      <button type="button" className="history-back" onClick={onBack}>
        <IconChevronLeft /> 返回记录列表
      </button>

      <div className="history-detail-summary card">
        <div className="history-detail-header">
          <div>
            <div className="history-detail-time">{record.completedAt}</div>
            <div className="history-detail-flow">
              <span>{record.source.name}</span>
              <VcsBadge type={record.source.type} />
              <span className="history-arrow">
                <IconArrow />
              </span>
              <span>{record.target.name}</span>
              <VcsBadge type={record.target.type} />
            </div>
          </div>
          <span className="badge badge-add history-status">已完成</span>
        </div>
        <div className="history-detail-grid">
          <div className="history-detail-cell">
            <span className="label">源仓库</span>
            <span className="value">{record.source.path}</span>
            <span className="sub">{record.source.branch}</span>
          </div>
          <div className="history-detail-cell">
            <span className="label">目标仓库</span>
            <span className="value">{record.target.path}</span>
            <span className="sub">{record.target.branch}</span>
          </div>
          <div className="history-detail-cell">
            <span className="label">提交</span>
            <span className="value">{record.commits.length} 条</span>
          </div>
          <div className="history-detail-cell">
            <span className="label">文件变更</span>
            <span className="value">
              {record.files.length} 个（+{stats.adds} / ~{stats.mods} / −{stats.dels}）
            </span>
            <span className="sub">
              +{stats.additions} / −{stats.deletions} 行
            </span>
          </div>
          {record.conflictsResolved > 0 && (
            <div className="history-detail-cell">
              <span className="label">冲突</span>
              <span className="value">已解决 {record.conflictsResolved} 处</span>
            </div>
          )}
        </div>
      </div>

      <div className="history-detail-tabs">
        <button
          type="button"
          className={`history-detail-tab${detailTab === "commits" ? " active" : ""}`}
          onClick={() => setDetailTab("commits")}
        >
          迁移的提交
        </button>
        <button
          type="button"
          className={`history-detail-tab${detailTab === "files" ? " active" : ""}`}
          onClick={() => setDetailTab("files")}
        >
          文件变更
        </button>
      </div>

      {detailTab === "commits" ? (
        <div className="card">
          <div className="commit-list">
            {record.commits.map((c) => (
              <CommitRowReadonly key={c.id} commit={c} />
            ))}
          </div>
        </div>
      ) : (
        <div className="history-preview-layout">
          <FileTree files={record.files} activeId={activeFileId} onSelect={setActiveFileId} />
          <DiffView file={activeFile} />
        </div>
      )}
    </div>
  );
}

function RepoManagement({ repos, onAdd, onEdit, onRemove }) {
  const [search, setSearch] = useState("");
  const filtered = repos.filter(
    (r) =>
      r.name.toLowerCase().includes(search.toLowerCase()) ||
      r.path.toLowerCase().includes(search.toLowerCase()),
  );
  return (
    <div className="repo-mgmt" data-screen-label="仓库管理">
      <div className="repo-mgmt-toolbar">
        <input
          className="search-input"
          placeholder="搜索仓库…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <button className="btn btn-primary" onClick={onAdd}>
          <IconPlus /> 添加仓库
        </button>
      </div>
      <div className="card">
        <table className="repo-table">
          <thead>
            <tr>
              <th>名称</th>
              <th>路径</th>
              <th>类型</th>
              <th>分支 / 路径</th>
              <th>最近使用</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((repo) => (
              <tr key={repo.id}>
                <td style={{ fontWeight: 600 }}>{repo.name}</td>
                <td style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--text-muted)" }}>
                  {repo.path}
                </td>
                <td>
                  <VcsBadge type={repo.type} />
                </td>
                <td>{repo.branch}</td>
                <td style={{ color: "var(--text-muted)", fontSize: 12 }}>{repo.lastUsed}</td>
                <td>
                  <div className="actions">
                    <button className="icon-btn" title="编辑" onClick={() => onEdit(repo)}>
                      <IconEdit />
                    </button>
                    <button
                      className="icon-btn danger"
                      title="移除"
                      onClick={() => onRemove(repo.id)}
                    >
                      <IconTrash />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

Object.assign(window, {
  VcsBadge,
  StatusBadge,
  TitleBar,
  StepRail,
  RepoCard,
  CommitRow,
  CommitPicker,
  CommitRowReadonly,
  FileTree,
  DiffView,
  EditorOpenMenu,
  EditorAppModal,
  EditorSettings,
  Toast,
  RepoModal,
  RepoManagement,
  MigrationHistory,
  MigrationDetail,
});
