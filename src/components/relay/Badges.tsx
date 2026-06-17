import type { VcsKind, FileStatus } from "../../lib/types";
import { STATUS_LABELS } from "../../lib/constants";
import { IconGit, IconSvn } from "./icons";

export function VcsBadge({ type }: { type: VcsKind }) {
  const isGit = type === "git";
  return (
    <span className={`badge ${isGit ? "badge-git" : "badge-svn"}`}>
      {isGit ? <IconGit /> : <IconSvn />}
      {isGit ? "Git" : "SVN"}
    </span>
  );
}

export function StatusBadge({ status }: { status: FileStatus }) {
  const cls = { add: "badge-add", mod: "badge-mod", del: "badge-del" }[status];
  return <span className={`badge ${cls}`}>{STATUS_LABELS[status]}</span>;
}
