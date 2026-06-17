const { useState, useMemo } = React;

function App() {
  const [repos, setRepos] = useState(INITIAL_REPOS);
  const [step, setStep] = useState("source");
  const [showRepos, setShowRepos] = useState(false);
  const [sourceId, setSourceId] = useState(null);
  const [targetId, setTargetId] = useState(null);
  const [selectedCommits, setSelectedCommits] = useState(new Set(["c1", "c2"]));
  const [activeFile, setActiveFile] = useState("f1");
  const [activeConflictId, setActiveConflictId] = useState("cf1");
  const [resolvedConflicts, setResolvedConflicts] = useState(new Set());
  const [migrated, setMigrated] = useState(false);
  const [modal, setModal] = useState(null);
  const [toast, setToast] = useState(null);
  const [settingsTab, setSettingsTab] = useState("repos");
  const [editors, setEditors] = useState(EDITOR_PRESETS);
  const [selectedEditorId, setSelectedEditorId] = useState("vscode");
  const [migrationHistory, setMigrationHistory] = useState(INITIAL_MIGRATION_HISTORY);
  const [historyDetailId, setHistoryDetailId] = useState(null);
  const [lastMigrationId, setLastMigrationId] = useState(null);

  const source = repos.find((r) => r.id === sourceId);
  const target = repos.find((r) => r.id === targetId);

  const completed = useMemo(() => {
    const s = new Set();
    if (sourceId) s.add("source");
    if (selectedCommits.size > 0) s.add("commits");
    if (selectedCommits.size > 0) s.add("preview");
    if (targetId) s.add("target");
    if (migrated) s.add("migrate");
    return s;
  }, [sourceId, selectedCommits, targetId, migrated]);

  const toggleCommit = (id) => {
    setSelectedCommits((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectCommits = (ids, select) => {
    setSelectedCommits((prev) => {
      const next = new Set(prev);
      ids.forEach((id) => {
        if (select) next.add(id);
        else next.delete(id);
      });
      return next;
    });
  };

  const goStep = (id) => {
    setShowRepos(false);
    setStep(id);
  };

  const handleManageRepos = () => {
    setShowRepos(true);
    setSettingsTab("repos");
    setHistoryDetailId(null);
  };

  const openMigrationHistory = (tab = "history", detailId = null) => {
    setShowRepos(true);
    setSettingsTab(tab);
    setHistoryDetailId(detailId);
  };

  const executeMigration = () => {
    if (!source || !target) return;
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
      commits: COMMITS.filter((c) => selectedCommits.has(c.id)),
      files: FILE_CHANGES,
      conflictsResolved: resolvedConflicts.size,
      status: "success",
    };
    setMigrationHistory((list) => [record, ...list]);
    setLastMigrationId(record.id);
    setMigrated(true);
  };

  const saveRepo = (form) => {
    if (modal?.mode === "edit") {
      setRepos((rs) => rs.map((r) => r.id === modal.repo.id ? { ...r, ...form } : r));
    } else {
      setRepos((rs) => [...rs, { ...form, id: `r${Date.now()}`, lastUsed: "刚刚" }]);
    }
    setModal(null);
  };

  const removeRepo = (id) => {
    setRepos((rs) => rs.filter((r) => r.id !== id));
    if (sourceId === id) setSourceId(null);
    if (targetId === id) setTargetId(null);
  };

  const canNext = () => {
    if (showRepos) return false;
    if (step === "source") return !!sourceId;
    if (step === "commits") return selectedCommits.size > 0;
    if (step === "preview") return true;
    if (step === "target") return !!targetId && targetId !== sourceId;
    return false;
  };

  const nextStep = () => {
    const idx = STEPS.findIndex((s) => s.id === step);
    if (idx < STEPS.length - 1) setStep(STEPS[idx + 1].id);
  };

  const prevStep = () => {
    if (showRepos) { setShowRepos(false); return; }
    const idx = STEPS.findIndex((s) => s.id === step);
    if (idx > 0) setStep(STEPS[idx - 1].id);
  };

  const hasConflicts = CONFLICTS.some((c) => !resolvedConflicts.has(c.id));

  const selectedEditor = editors.find((e) => e.id === selectedEditorId);

  const openInEditor = (cf, editorId) => {
    const id = editorId || selectedEditorId;
    if (editorId) setSelectedEditorId(editorId);
    const editor = editors.find((e) => e.id === id) || selectedEditor;
    const fullPath = target
      ? `${target.path}\\${cf.path.replace(/\//g, "\\")}`
      : cf.path;
    setActiveConflictId(cf.id);
    setToast(
      <span>已用 <strong>{editor?.name ?? "编辑器"}</strong> 打开 <code>{fullPath}</code></span>
    );
  };

  const addCustomEditor = ({ name, exe }) => {
    const id = `custom-${Date.now()}`;
    const entry = { id, name, exe, kind: "custom", custom: true };
    setEditors((list) => [...list, entry]);
    setSelectedEditorId(id);
    setModal(null);
    setToast(<span>已添加编辑器 <strong>{name}</strong></span>);
  };

  const removeEditor = (id) => {
    setEditors((list) => list.filter((e) => e.id !== id));
    if (selectedEditorId === id) setSelectedEditorId(EDITOR_PRESETS[0].id);
  };

  const markConflictResolved = (id) => {
    setResolvedConflicts((prev) => {
      const next = new Set([...prev, id]);
      const remaining = CONFLICTS.find((c) => !next.has(c.id));
      if (remaining) setActiveConflictId(remaining.id);
      return next;
    });
  };

  const renderContent = () => {
    if (showRepos) {
      return (
        <div className="main-screen" data-screen-label="设置">
          <div className="main-header">
            <div>
              <h1>设置</h1>
              <p>管理仓库、编辑器与迁移记录</p>
            </div>
          </div>
          <div className="main-content main-content--scroll">
            <div className="settings-tabs">
              <button
                type="button"
                className={`settings-tab${settingsTab === "repos" ? " active" : ""}`}
                onClick={() => { setSettingsTab("repos"); setHistoryDetailId(null); }}
              >
                仓库
              </button>
              <button
                type="button"
                className={`settings-tab${settingsTab === "editors" ? " active" : ""}`}
                onClick={() => { setSettingsTab("editors"); setHistoryDetailId(null); }}
              >
                编辑器
              </button>
              <button
                type="button"
                className={`settings-tab${settingsTab === "history" ? " active" : ""}`}
                onClick={() => { setSettingsTab("history"); setHistoryDetailId(null); }}
              >
                迁移记录
              </button>
            </div>
            {settingsTab === "repos" ? (
              <RepoManagement
                repos={repos}
                onAdd={() => setModal({ mode: "add" })}
                onEdit={(repo) => setModal({ mode: "edit", repo })}
                onRemove={removeRepo}
              />
            ) : settingsTab === "editors" ? (
              <EditorSettings
                editors={editors}
                selectedId={selectedEditorId}
                onSelect={setSelectedEditorId}
                onAdd={() => setModal({ mode: "editor" })}
                onRemove={removeEditor}
              />
            ) : historyDetailId ? (
              <MigrationDetail
                record={migrationHistory.find((r) => r.id === historyDetailId)}
                onBack={() => setHistoryDetailId(null)}
              />
            ) : (
              <MigrationHistory
                records={migrationHistory}
                onSelect={setHistoryDetailId}
              />
            )}
          </div>
        </div>
      );
    }

    if (step === "source") {
      return (
        <div className="main-screen" data-screen-label="源仓库">
          <div className="main-header">
            <div>
              <h1>选择源仓库</h1>
              <p>从已保存的仓库中选择提交来源，或添加新仓库</p>
            </div>
            <button className="btn btn-ghost" onClick={() => setModal({ mode: "add" })}>
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
    }

    if (step === "commits") {
      return (
        <div className="main-screen" data-screen-label="选择提交">
          <div className="main-header">
            <div>
              <h1>选择提交</h1>
              <p>
                来自 <strong>{source?.name}</strong> · 最近 {COMMITS.length} 条提交，已选 {selectedCommits.size} 条
              </p>
            </div>
          </div>
          <div className="main-content main-content--scroll">
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
    }

    if (step === "preview") {
      const file = FILE_CHANGES.find((f) => f.id === activeFile);
      const totalAdd = FILE_CHANGES.reduce((s, f) => s + f.additions, 0);
      const totalDel = FILE_CHANGES.reduce((s, f) => s + f.deletions, 0);
      return (
        <div className="main-screen" data-screen-label="变更预览">
          <div className="main-header">
            <div>
              <h1>变更预览</h1>
              <p>{selectedCommits.size} 条提交 · 以文件为单位查看增删改</p>
            </div>
          </div>
          <div className="main-content">
            <div className="summary-bar">
              <div className="summary-stat"><span className="badge badge-add">新增</span> <strong>{FILE_CHANGES.filter(f => f.status === "add").length}</strong> 个文件</div>
              <div className="summary-stat"><span className="badge badge-mod">修改</span> <strong>{FILE_CHANGES.filter(f => f.status === "mod").length}</strong> 个文件</div>
              <div className="summary-stat"><span className="badge badge-del">删除</span> <strong>{FILE_CHANGES.filter(f => f.status === "del").length}</strong> 个文件</div>
              <div className="summary-stat" style={{ marginLeft: "auto", color: "var(--text-muted)" }}>
                +{totalAdd} / −{totalDel} 行
              </div>
            </div>
            <div className="preview-layout">
              <FileTree files={FILE_CHANGES} activeId={activeFile} onSelect={setActiveFile} />
              <DiffView file={file} />
            </div>
          </div>
        </div>
      );
    }

    if (step === "target") {
      const available = repos.filter((r) => r.id !== sourceId);
      return (
        <div className="main-screen" data-screen-label="目标仓库">
          <div className="main-header">
            <div>
              <h1>选择目标仓库</h1>
              <p>提交将迁移至此仓库，迁移前会自动检测冲突</p>
            </div>
          </div>
          <div className="main-content main-content--scroll">
            <div className="migrate-flow">
              <div className="migrate-node">
                <div className="migrate-node-label">源</div>
                <div className="migrate-node-name">{source?.name}</div>
                <div className="migrate-node-path">{source?.path}</div>
                <div style={{ marginTop: 8 }}><VcsBadge type={source?.type} /></div>
              </div>
              <div className="migrate-connector"><IconArrow /></div>
              <div className="migrate-node" style={{ borderStyle: targetId ? "solid" : "dashed", opacity: targetId ? 1 : 0.6 }}>
                <div className="migrate-node-label">目标</div>
                {target ? (
                  <>
                    <div className="migrate-node-name">{target.name}</div>
                    <div className="migrate-node-path">{target.path}</div>
                    <div style={{ marginTop: 8 }}><VcsBadge type={target.type} /></div>
                  </>
                ) : (
                  <div style={{ color: "var(--text-muted)", marginTop: 8 }}>请选择下方目标仓库</div>
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
          </div>
        </div>
      );
    }

    if (step === "migrate") {
      if (migrated) {
        return (
          <div className="main-screen" data-screen-label="迁移完成">
            <div className="main-content main-content--scroll">
              <div className="success-panel">
                <div className="success-icon"><IconSuccess /></div>
                <h2>迁移完成</h2>
                <p>
                  已将 {selectedCommits.size} 条提交从 <strong>{source?.name}</strong> 成功应用到 <strong>{target?.name}</strong>。
                  共变更 {FILE_CHANGES.length} 个文件。
                </p>
                <div className="success-actions">
                  <button className="btn btn-ghost" onClick={() => openMigrationHistory("history", lastMigrationId)}>
                    查看本次记录
                  </button>
                  <button className="btn btn-primary" onClick={() => {
                    setMigrated(false);
                    setStep("source");
                    setSourceId(null);
                    setTargetId(null);
                    setSelectedCommits(new Set());
                    setResolvedConflicts(new Set());
                    setLastMigrationId(null);
                  }}>
                    开始新的迁移
                  </button>
                </div>
              </div>
            </div>
          </div>
        );
      }

      return (
        <div className="main-screen" data-screen-label="迁移确认">
          <div className="main-header">
            <div>
              <h1>确认迁移</h1>
              <p>检测目标仓库是否存在冲突，确认后执行迁移</p>
            </div>
          </div>
          <div className={`main-content${hasConflicts ? "" : " main-content--scroll"}`}>
            {hasConflicts ? (
              <>
                <div className="conflict-banner">
                  <div className="conflict-banner-icon"><IconWarn /></div>
                  <div>
                    <h3>发现 {CONFLICTS.length} 处冲突</h3>
                    <p>点击左侧冲突文件即可在右侧查看差异；需手工修改时，用外部编辑器打开，逐文件标记已解决。</p>
                  </div>
                </div>
                <ConflictWorkspace
                  conflicts={CONFLICTS}
                  activeId={activeConflictId}
                  resolvedIds={resolvedConflicts}
                  target={target}
                  editors={editors}
                  selectedEditorId={selectedEditorId}
                  onSelectEditor={setSelectedEditorId}
                  onBrowseEditor={() => setModal({ mode: "editor" })}
                  onSelect={setActiveConflictId}
                  onOpenWithEditor={openInEditor}
                  onMarkResolved={markConflictResolved}
                />
              </>
            ) : resolvedConflicts.size > 0 || CONFLICTS.length === 0 ? (
              <div className="card" style={{ padding: 24 }}>
                <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-secondary)" }}>源仓库</span>
                    <span>{source?.name} <VcsBadge type={source?.type} /></span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-secondary)" }}>目标仓库</span>
                    <span>{target?.name} <VcsBadge type={target?.type} /></span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-secondary)" }}>提交数量</span>
                    <span>{selectedCommits.size} 条</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-secondary)" }}>文件变更</span>
                    <span>{FILE_CHANGES.length} 个文件（+{FILE_CHANGES.filter(f=>f.status==="add").length} / ~{FILE_CHANGES.filter(f=>f.status==="mod").length} / −{FILE_CHANGES.filter(f=>f.status==="del").length}）</span>
                  </div>
                  <div style={{ borderTop: "1px solid var(--border)", marginTop: 8, paddingTop: 16, color: "var(--add)", display: "flex", alignItems: "center", gap: 8 }}>
                    <IconCheck /> {resolvedConflicts.size > 0 ? "所有冲突已解决，可以执行迁移" : "未检测到冲突，可以安全迁移"}
                  </div>
                </div>
              </div>
            ) : null}
          </div>
        </div>
      );
    }

    return null;
  };

  const footerHint = () => {
    if (showRepos) {
      if (settingsTab === "repos") return `${repos.length} 个已保存仓库`;
      if (settingsTab === "editors") return `默认编辑器：${selectedEditor?.name ?? "未选择"}`;
      if (historyDetailId) return "迁移记录明细";
      return `${migrationHistory.length} 条迁移记录`;
    }
    if (step === "source" && !sourceId) return "请选择一个源仓库";
    if (step === "commits") return `已选择 ${selectedCommits.size} 条提交`;
    if (step === "preview") return `${FILE_CHANGES.length} 个文件待迁移`;
    if (step === "target" && !targetId) return "请选择目标仓库";
    if (step === "target" && targetId === sourceId) return "目标不能与源相同";
    if (step === "migrate" && hasConflicts) {
      const pending = CONFLICTS.length - resolvedConflicts.size;
      return `还有 ${pending} 处冲突待解决`;
    }
    return "";
  };

  return (
    <div className="app-shell">
      <TitleBar />
      <div className="body">
        <StepRail
          steps={STEPS}
          current={showRepos ? null : step}
          completed={completed}
          onStep={goStep}
          onManageRepos={handleManageRepos}
          showRepos={showRepos}
        />
        <div className="main-area">
          {renderContent()}
          {!migrated && (
            <div className="main-footer">
              <span className="footer-left">{footerHint()}</span>
              <div className="footer-actions">
                {!showRepos && step !== "source" && (
                  <button className="btn btn-ghost" onClick={prevStep}>上一步</button>
                )}
                {showRepos ? (
                  <button className="btn btn-primary" onClick={() => setShowRepos(false)}>返回迁移</button>
                ) : step === "migrate" ? (
                  <button className="btn btn-primary" disabled={hasConflicts} onClick={executeMigration}>
                    执行迁移
                  </button>
                ) : (
                  <button className="btn btn-primary" disabled={!canNext()} onClick={nextStep}>
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
          onSave={addCustomEditor}
        />
      ) : modal ? (
        <RepoModal
          repo={modal.mode === "edit" ? modal.repo : null}
          onClose={() => setModal(null)}
          onSave={saveRepo}
        />
      ) : null}

      {toast && <Toast message={toast} onDone={() => setToast(null)} />}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
