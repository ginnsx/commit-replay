import { memo } from "react";
import type { CommitListItem, CommitSnapshot } from "../../lib/types";
import { IconCheck } from "./icons";

export const CommitRow = memo(function CommitRow({
  commit,
  selected,
  relayed,
  onToggle,
}: {
  commit: CommitListItem;
  selected: boolean;
  relayed?: boolean;
  onToggle: (id: string) => void;
}) {
  return (
    <div
      className={`commit-row${selected ? " selected" : ""}${relayed ? " relayed" : ""}`}
      onClick={() => onToggle(commit.id)}
    >
      <div className="commit-check">{selected && <IconCheck />}</div>
      <span className="commit-hash">{commit.hash}</span>
      <span className="commit-msg">
        {commit.msg}
        {relayed && <span className="badge badge-relayed">已提交</span>}
      </span>
      <div className="commit-meta">
        <span>{commit.author}</span>
        <span>{commit.date}</span>
        <span>{commit.files} 个文件</span>
      </div>
    </div>
  );
});

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
