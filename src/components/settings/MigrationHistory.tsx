import { useState } from "react";
import type { FileChangeView, MigrationRecord } from "../../lib/types";
import { migrationStats } from "../../lib/constants";
import { VcsBadge } from "../relay/Badges";
import { CommitRowReadonly } from "../relay/CommitRow";
import { DiffView } from "../relay/DiffView";
import { FileTree } from "../relay/FileTree";
import { IconArrow, IconChevronLeft, IconChevronRight } from "../relay/icons";

export function MigrationHistory({
  records,
  onSelect,
}: {
  records: MigrationRecord[];
  onSelect: (id: string) => void;
}) {
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
    <div className="migration-history">
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

export function MigrationDetail({
  record,
  onBack,
}: {
  record?: MigrationRecord | null;
  onBack: () => void;
}) {
  const [detailTab, setDetailTab] = useState<"commits" | "files">("commits");
  const [activeFileId, setActiveFileId] = useState<string | null>(record?.files[0]?.id ?? null);

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
  const activeFile: FileChangeView | undefined = record.files.find((f) => f.id === activeFileId);

  return (
    <div className="migration-detail">
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
