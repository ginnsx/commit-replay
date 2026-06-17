import type { WizardStep } from "../lib/types";

export const COMMIT_PAGE_SIZE = 500;
export const COMMIT_FETCH_SIZE = 500;

export const STEPS: { id: WizardStep; label: string; num: number }[] = [
  { id: "source", label: "源仓库", num: 1 },
  { id: "commits", label: "选择提交", num: 2 },
  { id: "target", label: "目标仓库", num: 3 },
  { id: "preview", label: "变更预览", num: 4 },
  { id: "migrate", label: "迁移", num: 5 },
];

export const STATUS_LABELS: Record<string, string> = {
  add: "新增",
  mod: "修改",
  del: "删除",
};

export function defaultSvnMappings(branch: string) {
  const trimmed = branch.trim().replace(/^\/+|\/+$/g, "");
  const fromBranch = trimmed ? `/${trimmed}` : "/trunk";
  return [
    { from: fromBranch, to: "." },
    { from: "/", to: "." },
  ];
}

export function splitFilePath(path: string) {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (i < 0) return { dir: "", name: path };
  return { dir: path.slice(0, i + 1), name: path.slice(i + 1) };
}

export function migrationStats(files: { status: string; additions: number; deletions: number }[]) {
  return {
    adds: files.filter((f) => f.status === "add").length,
    mods: files.filter((f) => f.status === "mod").length,
    dels: files.filter((f) => f.status === "del").length,
    additions: files.reduce((s, f) => s + f.additions, 0),
    deletions: files.reduce((s, f) => s + f.deletions, 0),
  };
}

export function isSupportedCombo(_sourceType: string, _targetType: string) {
  return true;
}

export function defaultGitMappings() {
  return [{ from: ".", to: "." }];
}

export function defaultMappingsForSource(sourceType: string, branch: string) {
  if (sourceType === "svn") return defaultSvnMappings(branch);
  return defaultGitMappings();
}

export function targetFilePath(targetPath: string, filePath: string) {
  return `${targetPath}\\${filePath.replace(/\//g, "\\")}`;
}
