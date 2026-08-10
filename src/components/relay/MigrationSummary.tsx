import type { MigrationMode, PathMapping, Repo } from "../../lib/types";

const MODE_LABELS: Record<MigrationMode, string> = {
  incremental_first: "增量优先",
  commit_result: "以提交结果为准",
  strict_replay: "严格复现",
};

export function MigrationSummary({
  source,
  target,
  commitCount,
  fileCount,
  migrationMode,
  squashCommits,
  mappings,
  customMapping,
  expanded,
  onToggle,
}: {
  source?: Repo;
  target?: Repo;
  commitCount: number;
  fileCount: number;
  migrationMode: MigrationMode;
  squashCommits: boolean;
  mappings: PathMapping[];
  customMapping: boolean;
  expanded: boolean;
  onToggle: () => void;
}) {
  const route = `${source?.name ?? "未选择"} → ${target?.name ?? "未选择"}`;
  const mappingLabel = customMapping
    ? mappings.map((mapping) => `${mapping.from}→${mapping.to}`).join("，")
    : "保持目录结构";

  return (
    <section
      className={`migration-summary card${expanded ? " expanded" : ""}`}
      aria-labelledby="migration-summary-title"
    >
      <div className="migration-summary-compact">
        <div className="migration-summary-route">
          <span className="migration-summary-eyebrow">执行摘要</span>
          <strong id="migration-summary-title" title={route}>
            {route}
          </strong>
        </div>
        <div className="migration-summary-facts" aria-label="迁移摘要">
          <span>{commitCount} 条提交</span>
          <span>{fileCount} 个文件</span>
          <span>{MODE_LABELS[migrationMode]}</span>
          <span>{squashCommits ? "合并提交" : "保留原提交"}</span>
        </div>
        <button
          type="button"
          className="btn btn-ghost migration-summary-toggle"
          aria-expanded={expanded}
          aria-controls="migration-summary-details"
          onClick={onToggle}
        >
          {expanded ? "收起详情" : "展开详情"}
        </button>
      </div>
      {expanded && (
        <div id="migration-summary-details" className="migration-summary-details">
          <dl>
            <div>
              <dt>目标路径</dt>
              <dd>
                <code>{target?.path ?? "未选择"}</code>
              </dd>
            </div>
            {target?.type === "git" && (
              <div>
                <dt>目标分支</dt>
                <dd>
                  从 <code>{target.branch}</code> 自动创建 <code>relay/时间戳</code>
                </dd>
              </div>
            )}
            <div>
              <dt>路径规则</dt>
              <dd>
                <code>{mappingLabel}</code>
              </dd>
            </div>
            <div>
              <dt>提交方式</dt>
              <dd>{squashCommits ? "合并为单次提交" : "保留原提交"}</dd>
            </div>
          </dl>
        </div>
      )}
    </section>
  );
}
