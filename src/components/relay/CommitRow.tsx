import type { CommitListItem, CommitSnapshot } from "../../lib/types";
import { IconCheck } from "./icons";

export function CommitRow({
  commit,
  selected,
  onToggle,
}: {
  commit: CommitListItem;
  selected: boolean;
  onToggle: () => void;
}) {
  return (
    <div className={`commit-row${selected ? " selected" : ""}`} onClick={onToggle}>
      <div className="commit-check">{selected && <IconCheck />}</div>
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

export function CommitRowReadonly({ commit }: { commit: CommitListItem | CommitSnapshot }) {
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
