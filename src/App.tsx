import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import type {
  AppErrorPayload,
  Editor,
  IntegrationItemView,
  IntegrationPlanResult,
  MigrationMode,
  MigrationRecord,
  PathMapping,
  PreviewMetaResult,
  Repo,
  RepoInput,
  WizardStep,
} from "./lib/types";
import { STEPS, COMMIT_FETCH_SIZE, defaultMappingsForSource, targetFilePath } from "./lib/constants";
import {
  buildPreviewMeta,
  buildIntegrationPlan,
  deleteEditor,
  deleteRepo,
  executeMigration,
  getDefaultEditorId,
  getRepoPairMappings,
  listEditors,
  listMigrations,
  listRepoCommits,
  listRepos,
  openFileInEditor,
  saveEditor,
  saveRepoPairMappings,
  saveRepo,
  setDefaultEditor,
  validateMigrationCombo,
} from "./lib/invoke";
import { TitleBar } from "./components/relay/TitleBar";
import { StepRail } from "./components/relay/StepRail";
import { BottomBar } from "./components/relay/BottomBar";
import { Toast } from "./components/relay/Toast";
import { RepoCard } from "./components/relay/RepoCard";
import { RepoModal, EditorAppModal } from "./components/relay/RepoModal";
import { CommitPicker } from "./components/relay/CommitPicker";
import { FileTree } from "./components/relay/FileTree";
import { DiffView } from "./components/relay/DiffView";
import { ConflictWorkspace } from "./components/relay/ConflictWorkspace";
import { PathMappingPanel } from "./components/relay/PathMappingPanel";
import { VcsBadge } from "./components/relay/Badges";
import { EditorSettings, RepoManagement } from "./components/settings/SettingsPanels";
import { MigrationDetail, MigrationHistory } from "./components/settings/MigrationHistory";
import {
  IconArrow,
  IconChevronRight,
  IconPlus,
  IconSuccess,
} from "./components/relay/icons";
import type { CommitListItem } from "./lib/types";

type ModalState =
  | { mode: "add" }
  | { mode: "edit"; repo: Repo }
  | { mode: "editor" }
  | null;

export default function App() {
  const [repos, setRepos] = useState<Repo[]>([]);
  const [editors, setEditors] = useState<Editor[]>([]);
  const [migrations, setMigrations] = useState<MigrationRecord[]>([]);
  const [selectedEditorId, setSelectedEditorId] = useState("vscode");

  const [step, setStep] = useState<WizardStep>("source");
  const [showRepos, setShowRepos] = useState(false);
  const [settingsTab, setSettingsTab] = useState<"repos" | "editors" | "history">("repos");
  const [historyDetailId, setHistoryDetailId] = useState<string | null>(null);

  const [sourceId, setSourceId] = useState<string | null>(null);
  const [targetId, setTargetId] = useState<string | null>(null);
  const [pathMappings, setPathMappings] = useState<PathMapping[]>([]);
  const [customMapping, setCustomMapping] = useState(false);
  const [selectedCommits, setSelectedCommits] = useState<Set<string>>(new Set());
  const [commits, setCommits] = useState<CommitListItem[]>([]);
  const [beforeCursor, setBeforeCursor] = useState<string | null>(null);
  const [commitsLoading, setCommitsLoading] = useState(false);
  const [commitsError, setCommitsError] = useState<AppErrorPayload | null>(null);
  const [lastBatchSize, setLastBatchSize] = useState(0);

  const [previewMeta, setPreviewMeta] = useState<PreviewMetaResult | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [activeFileId, setActiveFileId] = useState<string | null>(null);

  const [integrationPlan, setIntegrationPlan] = useState<IntegrationPlanResult | null>(null);
  const [migrationMode, setMigrationMode] = useState<MigrationMode>("incremental_first");
  const [squashCommits, setSquashCommits] = useState(false);
  const [squashCommitMessage, setSquashCommitMessage] = useState("");
  const [activeConflictId, setActiveConflictId] = useState<string | null>(null);
  const [acceptedReviewIds, setAcceptedReviewIds] = useState<Set<string>>(new Set());
  const [resolvedBlockedIds, setResolvedBlockedIds] = useState<Set<string>>(new Set());
  const [planLoading, setPlanLoading] = useState(false);

  const [migrated, setMigrated] = useState(false);
  const [lastMigrationId, setLastMigrationId] = useState<string | null>(null);
  const [migrating, setMigrating] = useState(false);

  const [modal, setModal] = useState<ModalState>(null);
  const [toast, setToast] = useState<ReactNode>(null);
  const [globalError, setGlobalError] = useState<string | null>(null);

  const attentionItems = useMemo(
    () =>
      integrationPlan?.items.filter(
        (i) => i.integrationStatus === "review" || i.integrationStatus === "blocked",
      ) ?? [],
    [integrationPlan],
  );

  const pendingReview = useMemo(
    () => attentionItems.filter((i) => i.integrationStatus === "review" && !acceptedReviewIds.has(i.id)),
    [attentionItems, acceptedReviewIds],
  );

  const pendingBlocked = useMemo(
    () => attentionItems.filter((i) => i.integrationStatus === "blocked" && !resolvedBlockedIds.has(i.id)),
    [attentionItems, resolvedBlockedIds],
  );

  const source = repos.find((r) => r.id === sourceId);
  const target = repos.find((r) => r.id === targetId);
  const selectedEditor = editors.find((e) => e.id === selectedEditorId);

  const canExecuteMigration =
    pendingReview.length === 0 &&
    pendingBlocked.length === 0 &&
    (!squashCommits || squashCommitMessage.trim().length > 0);

  const sourceRefs = useMemo(() => {
    if (step !== "preview" && step !== "migrate") return [];
    if (selectedCommits.size === 0) return [];
    const refs: string[] = [];
    for (const commit of commits) {
      if (selectedCommits.has(commit.id)) refs.push(commit.sourceRef);
    }
    return refs;
  }, [step, commits, selectedCommits]);

  const activeMappings = useMemo(() => {
    if (!source) return [];
    return customMapping ? pathMappings : defaultMappingsForSource(source.type, source.branch);
  }, [source, customMapping, pathMappings]);

  const completed = useMemo(() => {
    const s = new Set<WizardStep>();
    if (sourceId) s.add("source");
    if (selectedCommits.size > 0) s.add("commits");
    if (targetId) s.add("target");
    if (previewMeta) s.add("preview");
    if (migrated) s.add("migrate");
    return s;
  }, [sourceId, selectedCommits, targetId, previewMeta, migrated]);

  const refreshRepos = useCallback(async () => {
    setRepos(await listRepos());
  }, []);

  const refreshEditors = useCallback(async () => {
    const [list, def] = await Promise.all([listEditors(), getDefaultEditorId()]);
    setEditors(list);
    setSelectedEditorId(def);
  }, []);

  const refreshMigrations = useCallback(async () => {
    setMigrations(await listMigrations());
  }, []);

  useEffect(() => {
    refreshRepos().catch((e: AppErrorPayload) => setGlobalError(e.message));
    refreshEditors().catch(() => undefined);
    refreshMigrations().catch(() => undefined);
  }, [refreshRepos, refreshEditors, refreshMigrations]);

  const loadCommits = useCallback(
    async (reset = false) => {
      if (!sourceId) return;
      setCommitsLoading(true);
      setCommitsError(null);
      try {
        const cursor = reset ? null : beforeCursor;
        const batch = await listRepoCommits(sourceId, COMMIT_FETCH_SIZE, cursor);
        setCommits((prev) => (reset ? batch : [...prev, ...batch]));
        if (batch.length > 0) {
          const oldest = batch[batch.length - 1];
          setBeforeCursor(oldest.sourceRef);
        }
        setLastBatchSize(batch.length);
      } catch (e) {
        setCommitsError(e as AppErrorPayload);
      } finally {
        setCommitsLoading(false);
      }
    },
    [sourceId, beforeCursor],
  );

  useEffect(() => {
    if (step === "commits" && sourceId) {
      setBeforeCursor(null);
      setLastBatchSize(0);
      setCommits([]);
      loadCommits(true);
    }
  }, [step, sourceId]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!sourceId || !targetId || !source) return;
    let cancelled = false;
    getRepoPairMappings(sourceId, targetId)
      .then((saved) => {
        if (cancelled) return;
        if (saved) {
          setPathMappings(saved.pathMappings);
          setCustomMapping(saved.customMapping);
        } else {
          setPathMappings(defaultMappingsForSource(source.type, source.branch));
          setCustomMapping(false);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setPathMappings(defaultMappingsForSource(source.type, source.branch));
          setCustomMapping(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [sourceId, targetId, source?.type, source?.branch]);

  useEffect(() => {
    if (!sourceId || !targetId) return;
    setPreviewMeta(null);
  }, [sourceId, targetId, customMapping, pathMappings]);

  const persistPairMappings = useCallback(
    (mappings: PathMapping[], custom: boolean) => {
      if (!sourceId || !targetId) return;
      saveRepoPairMappings(sourceId, targetId, mappings, custom).catch(() => undefined);
    },
    [sourceId, targetId],
  );

  const handleMappingsChange = useCallback(
    (mappings: PathMapping[]) => {
      setPathMappings(mappings);
      persistPairMappings(mappings, true);
    },
    [persistPairMappings],
  );

  const handleCustomMappingChange = useCallback(
    (custom: boolean) => {
      setCustomMapping(custom);
      if (!source || !sourceId || !targetId) return;
      if (!custom) {
        const mappings = defaultMappingsForSource(source.type, source.branch);
        setPathMappings(mappings);
        persistPairMappings(mappings, false);
      } else {
        const mappings =
          pathMappings.length > 0
            ? pathMappings
            : defaultMappingsForSource(source.type, source.branch);
        persistPairMappings(mappings, true);
      }
    },
    [source, sourceId, targetId, pathMappings, persistPairMappings],
  );

  const activePreviewFile = useMemo(() => {
    if (!activeFileId || !previewMeta) return null;
    return previewMeta.files.find((f) => f.id === activeFileId) ?? null;
  }, [activeFileId, previewMeta]);

  const loadPreview = useCallback(async () => {
    if (!sourceId || !targetId || sourceRefs.length === 0) return;
    setPreviewLoading(true);
    setPreviewMeta(null);
    setActiveFileId(null);
    try {
      await validateMigrationCombo(sourceId, targetId);
      const meta = await buildPreviewMeta(sourceId, targetId, sourceRefs, activeMappings);
      setPreviewMeta(meta);
      if (meta.files.length > 0) {
        setActiveFileId(meta.files[0].id);
      }
    } catch (e) {
      const err = e as AppErrorPayload;
      setToast(<span>{err.message}</span>);
    } finally {
      setPreviewLoading(false);
    }
  }, [sourceId, targetId, sourceRefs, activeMappings]);

  useEffect(() => {
    if (step === "preview" && sourceId && targetId) {
      loadPreview();
    }
  }, [step, sourceId, targetId, loadPreview]);

  const loadIntegrationPlan = useCallback(async () => {
    if (!sourceId || !targetId || sourceRefs.length === 0) return;
    setPlanLoading(true);
    try {
      const plan = await buildIntegrationPlan(
        sourceId,
        targetId,
        sourceRefs,
        activeMappings,
        migrationMode,
      );
      setIntegrationPlan(plan);
      setAcceptedReviewIds(new Set());
      setResolvedBlockedIds(new Set());
      setActiveConflictId(
        plan.items.find((i) => i.integrationStatus === "blocked")?.id
          ?? plan.items.find((i) => i.integrationStatus === "review")?.id
          ?? plan.items[0]?.id
          ?? null,
      );
    } catch (e) {
      setToast(<span>{(e as AppErrorPayload).message}</span>);
    } finally {
      setPlanLoading(false);
    }
  }, [sourceId, targetId, sourceRefs, activeMappings, migrationMode]);

  useEffect(() => {
    if (step === "migrate" && !migrated) {
      loadIntegrationPlan();
    }
  }, [step, migrated, loadIntegrationPlan]);

  const canNext = () => {
    if (showRepos) return false;
    if (step === "source") return !!sourceId;
    if (step === "commits") return selectedCommits.size > 0;
    if (step === "preview") return !!previewMeta && previewMeta.files.length > 0;
    if (step === "target") {
      return !!targetId && targetId !== sourceId;
    }
    return false;
  };

  const nextStep = async () => {
    if (step === "target" && sourceId && targetId) {
      try {
        await validateMigrationCombo(sourceId, targetId);
      } catch (e) {
        setToast(<span>{(e as AppErrorPayload).message}</span>);
        return;
      }
    }
    const idx = STEPS.findIndex((s) => s.id === step);
    if (idx < STEPS.length - 1) setStep(STEPS[idx + 1].id);
  };

  const prevStep = () => {
    if (showRepos) {
      setShowRepos(false);
      return;
    }
    const idx = STEPS.findIndex((s) => s.id === step);
    if (idx > 0) setStep(STEPS[idx - 1].id);
  };

  const goStep = (id: WizardStep) => {
    setShowRepos(false);
    setStep(id);
  };

  const handleSaveRepo = async (input: RepoInput) => {
    const saved = await saveRepo(input);
    await refreshRepos();
    setModal(null);
    if (step === "target" && saved.id !== sourceId) {
      setTargetId(saved.id);
    } else if (step === "source" && !sourceId) {
      setSourceId(saved.id);
    }
  };

  const handleRemoveRepo = async (id: string) => {
    await deleteRepo(id);
    if (sourceId === id) setSourceId(null);
    if (targetId === id) setTargetId(null);
    await refreshRepos();
  };

  const handleAddEditor = async ({ name, exe }: { name: string; exe: string }) => {
    const id = `custom-${Date.now()}`;
    await saveEditor({ id, name, exe, kind: "custom", custom: true });
    await setDefaultEditor(id);
    await refreshEditors();
    setModal(null);
    setToast(
      <span>
        已添加编辑器 <strong>{name}</strong>
      </span>,
    );
  };

  const handleRemoveEditor = async (id: string) => {
    await deleteEditor(id);
    await refreshEditors();
  };

  const handleExecuteMigration = async () => {
    if (!sourceId || !targetId || !canExecuteMigration) return;
    setMigrating(true);
    try {
      const result = await executeMigration(
        sourceId,
        targetId,
        sourceRefs,
        resolvedBlockedIds.size,
        activeMappings,
        migrationMode,
        [...acceptedReviewIds],
        [...resolvedBlockedIds],
        squashCommits,
        squashCommitMessage.trim(),
      );
      setLastMigrationId(result.migrationId);
      setMigrated(true);
      await saveRepoPairMappings(sourceId, targetId, activeMappings, customMapping);
      await refreshMigrations();
    } catch (e) {
      setToast(<span>{(e as AppErrorPayload).message}</span>);
    } finally {
      setMigrating(false);
    }
  };

  const openInEditor = async (item: IntegrationItemView, editorId?: string) => {
    const id = editorId ?? selectedEditorId;
    if (editorId) {
      await setDefaultEditor(editorId);
      setSelectedEditorId(editorId);
    }
    const editor = editors.find((e) => e.id === id) ?? selectedEditor;
    const fullPath = target ? targetFilePath(target.path, item.path) : item.path;
    try {
      await openFileInEditor(id, fullPath);
      setActiveConflictId(item.id);
      setToast(
        <span>
          已用 <strong>{editor?.name ?? "编辑器"}</strong> 打开 <code>{fullPath}</code>
        </span>,
      );
    } catch (e) {
      setToast(<span>{(e as AppErrorPayload).message}</span>);
    }
  };

  const markBlockedResolved = (id: string) => {
    setResolvedBlockedIds((prev) => {
      const next = new Set([...prev, id]);
      const remaining = attentionItems.find(
        (c) => c.integrationStatus === "blocked" && !next.has(c.id),
      );
      if (remaining) setActiveConflictId(remaining.id);
      return next;
    });
  };

  const acceptReview = (id: string) => {
    setAcceptedReviewIds((prev) => {
      const next = new Set([...prev, id]);
      const remaining = attentionItems.find(
        (c) => c.integrationStatus === "review" && !next.has(c.id),
      );
      if (remaining) setActiveConflictId(remaining.id);
      return next;
    });
  };

  const acceptAllReview = () => {
    setAcceptedReviewIds(
      new Set(attentionItems.filter((i) => i.integrationStatus === "review").map((i) => i.id)),
    );
  };

  const handleMigrationModeChange = (mode: MigrationMode) => {
    setMigrationMode(mode);
  };

  const toggleCommit = useCallback((id: string) => {
    setSelectedCommits((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const selectCommits = useCallback((ids: string[], select: boolean) => {
    setSelectedCommits((prev) => {
      const next = new Set(prev);
      ids.forEach((id) => {
        if (select) next.add(id);
        else next.delete(id);
      });
      return next;
    });
  }, []);

  const resetMigration = () => {
    setMigrated(false);
    setStep("source");
    setSourceId(null);
    setTargetId(null);
    setSelectedCommits(new Set());
    setAcceptedReviewIds(new Set());
    setResolvedBlockedIds(new Set());
    setPreviewMeta(null);
    setIntegrationPlan(null);
    setLastMigrationId(null);
    setPathMappings([]);
    setCustomMapping(false);
    setSquashCommits(false);
    setSquashCommitMessage("");
  };

  const startNewMigration = () => {
    resetMigration();
    setShowRepos(false);
    setHistoryDetailId(null);
  };

  const footerHint = () => {
    if (showRepos) {
      if (settingsTab === "repos") return `${repos.length} 个已保存仓库`;
      if (settingsTab === "editors") return `默认编辑器：${selectedEditor?.name ?? "未选择"}`;
      if (historyDetailId) return "迁移记录明细";
      return `${migrations.length} 条迁移记录`;
    }
    if (step === "source" && !sourceId) return "请选择一个源仓库";
    if (step === "commits") return `已选择 ${selectedCommits.size} 条提交`;
    if (step === "preview") return `${previewMeta?.files.length ?? 0} 个文件待迁移`;
    if (step === "target" && !targetId) return "请选择目标仓库";
    if (step === "target" && targetId === sourceId) return "目标不能与源相同";
    if (step === "migrate" && !canExecuteMigration) {
      const parts: string[] = [];
      if (pendingReview.length > 0) parts.push(`${pendingReview.length} 个待确认`);
      if (pendingBlocked.length > 0) parts.push(`${pendingBlocked.length} 个待处理`);
      return parts.join(" · ");
    }
    return "";
  };

  const renderSettings = () => (
    <div className="main-screen">
      <div className="main-header">
        <div>
          <h1>设置</h1>
          <p>管理仓库、编辑器与迁移记录</p>
        </div>
      </div>
      <div className="main-content main-content--scroll">
        <div className="settings-tabs">
          {(["repos", "editors", "history"] as const).map((tab) => (
            <button
              key={tab}
              type="button"
              className={`settings-tab${settingsTab === tab ? " active" : ""}`}
              onClick={() => {
                setSettingsTab(tab);
                setHistoryDetailId(null);
              }}
            >
              {tab === "repos" ? "仓库" : tab === "editors" ? "编辑器" : "迁移记录"}
            </button>
          ))}
        </div>
        {settingsTab === "repos" ? (
          <RepoManagement
            repos={repos}
            onAdd={() => setModal({ mode: "add" })}
            onEdit={(repo) => setModal({ mode: "edit", repo })}
            onRemove={handleRemoveRepo}
          />
        ) : settingsTab === "editors" ? (
          <EditorSettings
            editors={editors}
            selectedId={selectedEditorId}
            onSelect={async (id) => {
              await setDefaultEditor(id);
              setSelectedEditorId(id);
            }}
            onAdd={() => setModal({ mode: "editor" })}
            onRemove={handleRemoveEditor}
          />
        ) : historyDetailId ? (
          <MigrationDetail
            record={migrations.find((r) => r.id === historyDetailId)}
            onBack={() => setHistoryDetailId(null)}
          />
        ) : (
          <MigrationHistory records={migrations} onSelect={setHistoryDetailId} />
        )}
      </div>
    </div>
  );

  const renderStep = () => {
    if (showRepos) return renderSettings();

    if (step === "source") {
      return (
        <div className="main-screen">
          <div className="main-header">
            <div>
              <h1>选择源仓库</h1>
              <p>从已保存的仓库中选择提交来源，或添加新仓库</p>
            </div>
            <button type="button" className="btn btn-ghost" onClick={() => setModal({ mode: "add" })}>
              <IconPlus /> 添加仓库
            </button>
          </div>
          <div className="main-content main-content--scroll">
            {globalError && <div className="conflict-banner">{globalError}</div>}
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
        <div className="main-screen">
          <div className="main-header">
            <div>
              <h1>选择提交</h1>
              <p>
                来自 <strong>{source?.name}</strong> · 最近 {commits.length} 条提交，已选{" "}
                {selectedCommits.size} 条
              </p>
            </div>
          </div>
          <div className="main-content main-content--commits">
            {commitsError && (
              <div className="conflict-banner">
                <p>{commitsError.message}</p>
                {commitsError.retryable && (
                  <button type="button" className="btn btn-ghost" onClick={() => loadCommits(true)}>
                    重试
                  </button>
                )}
              </div>
            )}
            <CommitPicker
              commits={commits}
              selectedIds={selectedCommits}
              onToggle={toggleCommit}
              onSelectMany={selectCommits}
              hasRemoteMore={lastBatchSize === COMMIT_FETCH_SIZE}
              onFetchRemote={() => loadCommits(false)}
              fetchingRemote={commitsLoading}
            />
          </div>
        </div>
      );
    }

    if (step === "preview") {
      const files = previewMeta?.files ?? [];
      return (
        <div className="main-screen main-screen--code">
          <div className="main-header">
            <div>
              <h1>变更预览</h1>
              <p>{selectedCommits.size} 条提交 · 合并后净变更</p>
            </div>
          </div>
          <div className="main-content">
            <div className="summary-bar">
              <div className="summary-stat">
                <span className="badge badge-add">新增</span>{" "}
                <strong>{previewLoading ? "…" : (previewMeta?.adds ?? 0)}</strong> 个文件
              </div>
              <div className="summary-stat">
                <span className="badge badge-mod">修改</span>{" "}
                <strong>{previewLoading ? "…" : (previewMeta?.mods ?? 0)}</strong> 个文件
              </div>
              <div className="summary-stat">
                <span className="badge badge-del">删除</span>{" "}
                <strong>{previewLoading ? "…" : (previewMeta?.dels ?? 0)}</strong> 个文件
              </div>
              <div className="summary-stat" style={{ marginLeft: "auto", color: "var(--text-muted)" }}>
                {previewLoading
                  ? "分析中…"
                  : `+${previewMeta?.totalAdditions ?? 0} / −${previewMeta?.totalDeletions ?? 0} 行`}
              </div>
            </div>
            <div className="preview-layout">
              {previewLoading ? (
                <div className="empty-state" style={{ gridColumn: "1 / -1" }}>
                  <p>正在分析合并后的变更…</p>
                </div>
              ) : files.length === 0 ? (
                <div className="empty-state" style={{ gridColumn: "1 / -1" }}>
                  <p>所选提交合并后无净变更</p>
                </div>
              ) : (
                <>
                  <FileTree files={files} activeId={activeFileId} onSelect={setActiveFileId} />
                  <DiffView file={activePreviewFile} />
                </>
              )}
            </div>
          </div>
        </div>
      );
    }

    if (step === "target") {
      const available = repos.filter((r) => r.id !== sourceId);
      return (
        <div className="main-screen">
          <div className="main-header">
            <div>
              <h1>选择目标仓库</h1>
              <p>从已保存的仓库中选择迁移目标，或添加新仓库</p>
            </div>
            <button type="button" className="btn btn-ghost" onClick={() => setModal({ mode: "add" })}>
              <IconPlus /> 添加仓库
            </button>
          </div>
          <div className="main-content main-content--scroll">
            <div className="migrate-flow">
              <div className="migrate-node">
                <div className="migrate-node-label">源</div>
                <div className="migrate-node-name">{source?.name}</div>
                <div className="migrate-node-path">{source?.path}</div>
                <div style={{ marginTop: 8 }}>{source && <VcsBadge type={source.type} />}</div>
              </div>
              <div className="migrate-connector">
                <IconArrow />
              </div>
              <div
                className="migrate-node"
                style={{ borderStyle: targetId ? "solid" : "dashed", opacity: targetId ? 1 : 0.6 }}
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
            {targetId && source && (
              <PathMappingPanel
                sourceType={source.type}
                targetType={target?.type ?? "git"}
                branch={source.branch}
                mappings={pathMappings}
                customMapping={customMapping}
                onMappingsChange={handleMappingsChange}
                onCustomMappingChange={handleCustomMappingChange}
              />
            )}
          </div>
        </div>
      );
    }

    if (step === "migrate") {
      if (migrated) {
        return (
          <div className="main-screen">
            <div className="main-content main-content--scroll">
              <div className="success-panel">
                <div className="success-icon">
                  <IconSuccess />
                </div>
                <h2>迁移完成</h2>
                <p>
                  已将 {selectedCommits.size} 条提交从 <strong>{source?.name}</strong> 成功应用到{" "}
                  <strong>{target?.name}</strong>。 共变更 {previewMeta?.files.length ?? 0} 个文件。
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
                  <button type="button" className="btn btn-primary" onClick={startNewMigration}>
                    开始新的迁移
                  </button>
                </div>
              </div>
            </div>
          </div>
        );
      }

      return (
        <div className="main-screen main-screen--code">
          <div className="main-header">
            <div>
              <h1>确认迁移</h1>
              <p>审查集成计划，确认后执行迁移</p>
            </div>
          </div>
          <div className="main-content main-content--commits">
            {planLoading ? (
              <div className="empty-state"><p>生成集成计划中…</p></div>
            ) : integrationPlan ? (
              <ConflictWorkspace
                items={integrationPlan.items}
                migrationMode={migrationMode}
                onMigrationModeChange={handleMigrationModeChange}
                squashCommits={squashCommits}
                onSquashCommitsChange={setSquashCommits}
                squashCommitMessage={squashCommitMessage}
                onSquashCommitMessageChange={setSquashCommitMessage}
                autoOkCount={integrationPlan.autoOkCount}
                reviewCount={integrationPlan.reviewCount}
                blockedCount={integrationPlan.blockedCount}
                activeId={activeConflictId}
                acceptedReviewIds={acceptedReviewIds}
                resolvedBlockedIds={resolvedBlockedIds}
                target={target}
                editors={editors}
                selectedEditorId={selectedEditorId}
                onSelectEditor={async (id) => {
                  await setDefaultEditor(id);
                  setSelectedEditorId(id);
                }}
                onBrowseEditor={() => setModal({ mode: "editor" })}
                onSelect={setActiveConflictId}
                onOpenWithEditor={openInEditor}
                onAcceptReview={acceptReview}
                onMarkResolved={markBlockedResolved}
                onAcceptAllReview={acceptAllReview}
                canExecute={canExecuteMigration}
              />
            ) : (
              <div className="empty-state"><p>无法生成集成计划</p></div>
            )}
          </div>
        </div>
      );
    }

    return null;
  };

  return (
    <div className="app-shell">
      <TitleBar />
      <div className="body">
        <StepRail
          current={showRepos ? null : step}
          completed={completed}
          onStep={goStep}
          onManageRepos={() => {
            setShowRepos(true);
            setSettingsTab("repos");
            setHistoryDetailId(null);
          }}
          showRepos={showRepos}
        />
        <div className="main-area">
          {renderStep()}
          {(!migrated || showRepos) && (
            <BottomBar
              left={footerHint()}
              showBack={!showRepos && step !== "source"}
              onBack={prevStep}
              onNext={showRepos ? undefined : step === "migrate" ? undefined : nextStep}
              nextDisabled={!canNext()}
              nextLabel={
                <>
                  下一步 <IconChevronRight />
                </>
              }
              primaryAction={
                showRepos ? (
                  migrated ? (
                    <>
                      <button type="button" className="btn btn-ghost" onClick={() => setShowRepos(false)}>
                        返回
                      </button>
                      <button type="button" className="btn btn-primary" onClick={startNewMigration}>
                        开始新的迁移
                      </button>
                    </>
                  ) : (
                    <button type="button" className="btn btn-primary" onClick={() => setShowRepos(false)}>
                      返回迁移
                    </button>
                  )
                ) : step === "migrate" ? (
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={!canExecuteMigration || migrating}
                    onClick={handleExecuteMigration}
                  >
                    {migrating ? "执行中…" : "执行迁移"}
                  </button>
                ) : undefined
              }
            />
          )}
        </div>
      </div>

      {modal?.mode === "editor" ? (
        <EditorAppModal onClose={() => setModal(null)} onSave={handleAddEditor} />
      ) : modal ? (
        <RepoModal
          repo={modal.mode === "edit" ? modal.repo : null}
          onClose={() => setModal(null)}
          onSave={handleSaveRepo}
        />
      ) : null}

      {toast && <Toast message={toast} onDone={() => setToast(null)} />}
    </div>
  );
}
