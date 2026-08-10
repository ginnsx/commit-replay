import { useState } from "react";
import type { Repo } from "../../lib/types";
import { IconEdit, IconPlus, IconTrash } from "../relay/icons";
import { VcsBadge } from "../relay/Badges";
import { RepoCard } from "../relay/RepoCard";

export function RepoManagement({
  repos,
  onAdd,
  onEdit,
  onRemove,
}: {
  repos: Repo[];
  onAdd: () => void;
  onEdit: (repo: Repo) => void;
  onRemove: (id: string) => void;
}) {
  const [search, setSearch] = useState("");
  const filtered = repos.filter(
    (r) =>
      r.name.toLowerCase().includes(search.toLowerCase()) ||
      r.path.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="repo-mgmt">
      <div className="repo-mgmt-toolbar">
        <input
          className="search-input"
          placeholder="搜索仓库…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <button type="button" className="btn btn-primary" onClick={onAdd}>
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
              <th />
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
                <td style={{ color: "var(--text-muted)", fontSize: 12 }}>{repo.lastUsed ?? "—"}</td>
                <td>
                  <div className="actions">
                    <button
                      type="button"
                      className="icon-btn"
                      title="编辑"
                      onClick={() => onEdit(repo)}
                    >
                      <IconEdit />
                    </button>
                    <button
                      type="button"
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

export function EditorSettings({
  editors,
  selectedId,
  onSelect,
  onAdd,
  onRemove,
}: {
  editors: { id: string; name: string; exe: string; custom: boolean }[];
  selectedId: string;
  onSelect: (id: string) => void;
  onAdd: () => void;
  onRemove: (id: string) => void;
}) {
  return (
    <div className="editor-settings">
      <p className="editor-settings-intro">
        选择默认外部编辑器，用于在冲突解决时打开目标仓库中的文件。
      </p>
      <div className="editor-option-list">
        {editors.map((e) => (
          <button
            key={e.id}
            type="button"
            className={`editor-option${selectedId === e.id ? " selected" : ""}`}
            onClick={() => onSelect(e.id)}
          >
            <span className="editor-option-radio" />
            <span className="editor-option-info">
              <div className="editor-option-name">{e.name}</div>
              <div className="editor-option-exe">{e.exe}</div>
            </span>
            {e.custom && (
              <button
                type="button"
                className="icon-btn danger"
                onClick={(ev) => {
                  ev.stopPropagation();
                  onRemove(e.id);
                }}
              >
                <IconTrash />
              </button>
            )}
          </button>
        ))}
      </div>
      <button type="button" className="btn btn-ghost" onClick={onAdd}>
        <IconPlus /> 添加编辑器应用
      </button>
    </div>
  );
}

export { RepoCard };
