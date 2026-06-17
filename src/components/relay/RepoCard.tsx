import type { Repo } from "../../lib/types";
import { VcsBadge } from "./Badges";

export function RepoCard({
  repo,
  selected,
  onClick,
}: {
  repo: Repo;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button" className={`repo-card${selected ? " selected" : ""}`} onClick={onClick}>
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
        <span>最近使用 {repo.lastUsed ?? "—"}</span>
      </div>
    </button>
  );
}
