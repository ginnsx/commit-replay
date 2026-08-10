import { useCallback, useEffect, useMemo, useState } from "react";
import type { GitRepoInfo, Repo, RepoInput, SvnWcInfo, VcsKind } from "../../lib/types";
import { pickFolder, probeGitRepo, probeSvnWc } from "../../lib/invoke";
import { IconClose, IconFolder } from "./icons";

interface Props {
  repo?: Repo | null;
  onClose: () => void;
  onSave: (input: RepoInput) => void;
}

export function RepoModal({ repo, onClose, onSave }: Props) {
  const isEdit = !!repo;
  const [form, setForm] = useState<RepoInput>(() =>
    repo
      ? {
          id: repo.id,
          name: repo.name,
          path: repo.path,
          type: repo.type,
          branch: repo.branch,
          svnUser: repo.svnUser,
          pathMappings: [],
        }
      : {
          name: "",
          path: "",
          type: "git",
          branch: "main",
          svnUser: "",
          svnPass: "",
          pathMappings: [],
        },
  );

  const [svnProbe, setSvnProbe] = useState<SvnWcInfo | null>(null);
  const [svnProbeError, setSvnProbeError] = useState<string | null>(null);
  const [svnProbing, setSvnProbing] = useState(false);

  const [gitProbe, setGitProbe] = useState<GitRepoInfo | null>(null);
  const [gitProbeError, setGitProbeError] = useState<string | null>(null);
  const [gitProbing, setGitProbing] = useState(false);

  const set = <K extends keyof RepoInput>(k: K, v: RepoInput[K]) =>
    setForm((f) => {
      const next = { ...f, [k]: v };
      if (k === "type") {
        const t = v as VcsKind;
        next.branch = t === "git" ? "main" : "trunk";
      }
      return next;
    });

  const applySvnProbe = useCallback((info: SvnWcInfo) => {
    setSvnProbe(info);
    setSvnProbeError(null);
    setForm((f) => ({ ...f, branch: info.branch }));
  }, []);

  const applyGitProbe = useCallback((info: GitRepoInfo) => {
    setGitProbe(info);
    setGitProbeError(null);
    setForm((f) => {
      const branch = info.branches.includes(f.branch) ? f.branch : info.branch;
      return { ...f, branch };
    });
  }, []);

  useEffect(() => {
    if (form.type !== "svn" || !form.path.trim()) {
      setSvnProbe(null);
      setSvnProbeError(null);
      return;
    }
    let cancelled = false;
    setSvnProbing(true);
    probeSvnWc(form.path.trim())
      .then((info) => {
        if (!cancelled) applySvnProbe(info);
      })
      .catch((e: { message?: string }) => {
        if (cancelled) return;
        setSvnProbe(null);
        setSvnProbeError(e.message ?? "无法识别 SVN 工作副本");
      })
      .finally(() => {
        if (!cancelled) setSvnProbing(false);
      });
    return () => {
      cancelled = true;
    };
  }, [form.path, form.type, applySvnProbe]);

  useEffect(() => {
    if (form.type !== "git" || !form.path.trim()) {
      setGitProbe(null);
      setGitProbeError(null);
      return;
    }
    let cancelled = false;
    setGitProbing(true);
    probeGitRepo(form.path.trim())
      .then((info) => {
        if (!cancelled) applyGitProbe(info);
      })
      .catch((e: { message?: string }) => {
        if (cancelled) return;
        setGitProbe(null);
        setGitProbeError(e.message ?? "无法识别 Git 仓库");
      })
      .finally(() => {
        if (!cancelled) setGitProbing(false);
      });
    return () => {
      cancelled = true;
    };
  }, [form.path, form.type, applyGitProbe]);

  const gitBranchOptions = useMemo(() => {
    const fromProbe = gitProbe?.branches ?? [];
    if (form.branch && !fromProbe.includes(form.branch)) {
      return [form.branch, ...fromProbe];
    }
    return fromProbe.length > 0 ? fromProbe : form.branch ? [form.branch] : [];
  }, [gitProbe, form.branch]);

  const browse = async () => {
    const p = await pickFolder();
    if (p) set("path", p);
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{isEdit ? "编辑仓库" : "添加仓库"}</h2>
          <button type="button" className="icon-btn" onClick={onClose}>
            <IconClose />
          </button>
        </div>
        <div className="modal-body">
          <div className="form-group">
            <label>显示名称</label>
            <input
              value={form.name}
              onChange={(e) => set("name", e.target.value)}
              placeholder="例如 payment-service"
            />
          </div>
          <div className="form-group">
            <label>本地目录</label>
            <div style={{ display: "flex", gap: 8 }}>
              <input
                style={{ flex: 1 }}
                value={form.path}
                onChange={(e) => set("path", e.target.value)}
                placeholder="D:\Projects\my-repo"
              />
              <button
                type="button"
                className="btn btn-ghost"
                style={{ flexShrink: 0 }}
                onClick={browse}
              >
                <IconFolder /> 浏览
              </button>
            </div>
            <p className="form-hint">选择已 checkout 到本地的 Git 或 SVN 项目文件夹</p>
          </div>
          <div className="form-group">
            <label>版本控制</label>
            <select value={form.type} onChange={(e) => set("type", e.target.value as VcsKind)}>
              <option value="git">Git</option>
              <option value="svn">SVN</option>
            </select>
          </div>

          {form.type === "git" && (
            <>
              {gitProbing && <p className="form-hint">正在识别 Git 仓库…</p>}
              {!gitProbing && gitProbe && (
                <div className="svn-wc-card ok">
                  <div className="svn-wc-card-title">已识别 Git 仓库</div>
                  <div className="svn-wc-row">
                    <span className="svn-wc-label">当前分支</span>
                    <span className="svn-wc-value">{gitProbe.branch}</span>
                  </div>
                  {gitProbe.remoteUrl && (
                    <div className="svn-wc-row">
                      <span className="svn-wc-label">远程</span>
                      <span className="svn-wc-value">{gitProbe.remoteUrl}</span>
                    </div>
                  )}
                </div>
              )}
              {!gitProbing && gitProbeError && form.path.trim() && (
                <div className="svn-wc-card warn">
                  <div className="svn-wc-card-title">未能识别 Git 仓库</div>
                  <p className="form-hint" style={{ margin: 0 }}>
                    {gitProbeError}
                  </p>
                </div>
              )}
              {gitBranchOptions.length > 0 && (
                <div className="form-group">
                  <label>默认分支</label>
                  <select
                    value={form.branch}
                    onChange={(e) => set("branch", e.target.value)}
                    disabled={gitProbing}
                  >
                    {gitBranchOptions.map((b) => (
                      <option key={b} value={b}>
                        {b}
                      </option>
                    ))}
                  </select>
                </div>
              )}
            </>
          )}

          {form.type === "svn" && (
            <>
              {svnProbing && <p className="form-hint">正在识别 SVN 工作副本…</p>}
              {!svnProbing && svnProbe && (
                <div className="svn-wc-card ok">
                  <div className="svn-wc-card-title">已识别 SVN 工作副本</div>
                  <div className="svn-wc-row">
                    <span className="svn-wc-label">检出点</span>
                    <span className="svn-wc-value">{svnProbe.branch}</span>
                  </div>
                  <div className="svn-wc-row">
                    <span className="svn-wc-label">服务器</span>
                    <span className="svn-wc-value">{svnProbe.relativeUrl}</span>
                  </div>
                </div>
              )}
              {!svnProbing && svnProbeError && form.path.trim() && (
                <div className="svn-wc-card warn">
                  <div className="svn-wc-card-title">未能识别 SVN 工作副本</div>
                  <p className="form-hint" style={{ margin: 0 }}>
                    {svnProbeError}
                  </p>
                </div>
              )}

              <div className="form-row">
                <div className="form-group">
                  <label>SVN 用户名</label>
                  <input
                    value={form.svnUser ?? ""}
                    onChange={(e) => set("svnUser", e.target.value)}
                  />
                </div>
                <div className="form-group">
                  <label>密码</label>
                  <input
                    type="password"
                    value={form.svnPass ?? ""}
                    onChange={(e) => set("svnPass", e.target.value)}
                    placeholder={isEdit && repo?.hasSvnPass ? "留空保持原密码" : "••••••••"}
                  />
                </div>
              </div>
              <p className="form-hint">凭据将加密保存在本地</p>
            </>
          )}
        </div>
        <div className="modal-footer">
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={!form.name.trim() || !form.path.trim()}
            onClick={() => onSave({ ...form, pathMappings: [] })}
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}

export function EditorAppModal({
  onClose,
  onSave,
}: {
  onClose: () => void;
  onSave: (data: { name: string; exe: string }) => void;
}) {
  const [name, setName] = useState("");
  const [exe, setExe] = useState("");

  const browse = async () => {
    const p = await pickFolder();
    if (p) setExe(p);
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>添加编辑器应用</h2>
          <button type="button" className="icon-btn" onClick={onClose}>
            <IconClose />
          </button>
        </div>
        <div className="modal-body">
          <div className="form-group">
            <label>显示名称</label>
            <input value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="form-group">
            <label>应用程序路径</label>
            <div style={{ display: "flex", gap: 8 }}>
              <input style={{ flex: 1 }} value={exe} onChange={(e) => setExe(e.target.value)} />
              <button type="button" className="btn btn-ghost" onClick={browse}>
                <IconFolder /> 浏览
              </button>
            </div>
          </div>
        </div>
        <div className="modal-footer">
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={!name.trim() || !exe.trim()}
            onClick={() => onSave({ name: name.trim(), exe: exe.trim() })}
          >
            添加
          </button>
        </div>
      </div>
    </div>
  );
}
