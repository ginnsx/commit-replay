import { useCallback, useEffect, useRef, useState } from "react";
import { compareMigrationFiles } from "../../lib/invoke";
import type {
  FileComparisonStatus,
  FileContentKind,
  FileChangeView,
  MigrationComparisonResult,
  MigrationFileComparison,
  MigrationRecord,
} from "../../lib/types";
import { DiffView } from "../relay/DiffView";

const STATUS_LABELS: Record<FileComparisonStatus, string> = {
  identical: "一致",
  format_only: "仅格式差异",
  different: "内容不同",
  source_missing: "源文件缺失",
  target_missing: "目标文件缺失",
  both_missing: "删除一致",
  unreadable: "无法读取",
};

const CONTENT_LABELS: Record<FileContentKind, string> = {
  text: "文本",
  binary: "二进制",
};

export function MigrationComparison({
  record,
  autoRun,
}: {
  record: MigrationRecord;
  autoRun: boolean;
}) {
  const [result, setResult] = useState<MigrationComparisonResult | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const autoRunRecordId = useRef<string | null>(null);

  const runComparison = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = await compareMigrationFiles(record.id);
      setResult(next);
      setActiveId(next.files[0]?.id ?? null);
    } catch (e) {
      setResult(null);
      setActiveId(null);
      setError((e as { message?: string }).message ?? "文件对比失败");
    } finally {
      setLoading(false);
    }
  }, [record.id]);

  useEffect(() => {
    if (!autoRun || autoRunRecordId.current === record.id) return;
    autoRunRecordId.current = record.id;
    void runComparison();
  }, [autoRun, record.id, runComparison]);

  if (!result && !loading && !error) {
    return (
      <section className="comparison-intro card">
        <div>
          <h3>源 / 目标文件对比</h3>
          <p>读取两边当前本地工作副本，不会更新仓库或修改文件。</p>
        </div>
        <button type="button" className="btn btn-primary" onClick={() => void runComparison()}>
          开始对比
        </button>
      </section>
    );
  }

  if (loading) {
    return (
      <section className="comparison-intro card">
        <div>
          <h3>正在对比文件…</h3>
          <p>正在读取本次迁移涉及的 source 和 target 文件。</p>
        </div>
      </section>
    );
  }

  if (error) {
    return (
      <section className="comparison-intro card comparison-error">
        <div>
          <h3>文件对比失败</h3>
          <p>{error}</p>
        </div>
        <button type="button" className="btn btn-ghost" onClick={() => void runComparison()}>
          重试
        </button>
      </section>
    );
  }

  if (!result?.available) {
    return (
      <section className="comparison-intro card comparison-error">
        <div>
          <h3>无法安全对比此记录</h3>
          <p>{result?.reason ?? "此记录缺少可用于对比的文件路径。"}</p>
        </div>
      </section>
    );
  }

  const activeFile = result.files.find((file) => file.id === activeId) ?? result.files[0];

  return (
    <section className="migration-comparison">
      <div className="comparison-toolbar">
        <div>
          <h3>源 / 目标文件对比</h3>
          <p>比较于 {result.comparedAt} · 当前本地工作副本</p>
        </div>
        <button type="button" className="btn btn-ghost" onClick={() => void runComparison()}>
          重新对比
        </button>
      </div>
      <ComparisonSummary result={result} />
      {result.files.length === 0 ? (
        <div className="empty-state card" style={{ padding: 40 }}>
          <p>本次迁移没有可对比的文件。</p>
        </div>
      ) : (
        <div className="history-preview-layout comparison-preview-layout">
          <ComparisonFileList
            files={result.files}
            activeId={activeFile?.id ?? null}
            onSelect={setActiveId}
          />
          <ComparisonDetail file={activeFile} />
        </div>
      )}
    </section>
  );
}

function ComparisonSummary({ result }: { result: MigrationComparisonResult }) {
  const { summary } = result;
  return (
    <div className="comparison-summary" aria-label="文件对比汇总">
      <span>{summary.total} 个文件</span>
      <span className="comparison-summary-identical">{summary.identical} 一致</span>
      <span className="comparison-summary-format">{summary.formatOnly} 格式差异</span>
      <span className="comparison-summary-different">{summary.different} 内容不同</span>
      <span className="comparison-summary-missing">{summary.missing} 缺失</span>
      <span className="comparison-summary-error">{summary.unreadable} 无法读取</span>
    </div>
  );
}

function ComparisonFileList({
  files,
  activeId,
  onSelect,
}: {
  files: MigrationFileComparison[];
  activeId: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="comparison-file-list">
      <div className="file-tree-header">
        <span>对比文件</span>
        <span>{files.length}</span>
      </div>
      <div className="comparison-file-list-body">
        {files.map((file) => (
          <button
            key={file.id}
            type="button"
            className={`comparison-file-item${activeId === file.id ? " active" : ""}`}
            onClick={() => onSelect(file.id)}
          >
            <span className={`comparison-status comparison-status-${file.status}`}>
              {STATUS_LABELS[file.status]}
            </span>
            <span className="comparison-file-path" title={file.targetPath}>
              {file.targetPath}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

function ComparisonDetail({ file }: { file?: MigrationFileComparison }) {
  if (!file) {
    return <div className="diff-panel" />;
  }
  if (file.contentKind === "binary") {
    return (
      <div className="comparison-binary-panel">
        <div className="comparison-detail-header">
          <div>
            <strong>{file.targetPath}</strong>
            <span>{STATUS_LABELS[file.status]} · 二进制文件</span>
          </div>
          <span className={`comparison-status comparison-status-${file.status}`}>
            {STATUS_LABELS[file.status]}
          </span>
        </div>
        <dl className="comparison-binary-meta">
          <div>
            <dt>源文件</dt>
            <dd>{file.sourcePath}</dd>
          </div>
          <div>
            <dt>目标文件</dt>
            <dd>{file.targetPath}</dd>
          </div>
          <div>
            <dt>源 SHA-256</dt>
            <dd>{file.sourceSha256 ?? "—"}</dd>
          </div>
          <div>
            <dt>目标 SHA-256</dt>
            <dd>{file.targetSha256 ?? "—"}</dd>
          </div>
          <div>
            <dt>大小</dt>
            <dd>
              {formatBytes(file.sourceSize)} → {formatBytes(file.targetSize)}
            </dd>
          </div>
        </dl>
        {file.message ? <p className="comparison-detail-message">{file.message}</p> : null}
      </div>
    );
  }

  const diffFile: FileChangeView = {
    id: file.id,
    path: `${file.sourcePath} ↔ ${file.targetPath}`,
    status: "mod",
    additions: 0,
    deletions: 0,
    diff: file.diff,
  };
  return (
    <div className="comparison-text-panel">
      <div className="comparison-detail-header">
        <div>
          <strong>{file.targetPath}</strong>
          <span>
            {STATUS_LABELS[file.status]}
            {file.contentKind ? ` · ${CONTENT_LABELS[file.contentKind]}` : ""}
          </span>
        </div>
        <span className={`comparison-status comparison-status-${file.status}`}>
          {STATUS_LABELS[file.status]}
        </span>
      </div>
      {file.message ? <p className="comparison-detail-message">{file.message}</p> : null}
      <DiffView file={diffFile} />
    </div>
  );
}

function formatBytes(bytes?: number) {
  if (bytes === undefined) return "—";
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}
