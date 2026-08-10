import type { PathMapping, VcsKind } from "../../lib/types";
import { defaultMappingsForSource } from "../../lib/constants";
import { IconPlus } from "./icons";

interface Props {
  sourceType: VcsKind;
  targetType: VcsKind;
  branch: string;
  mappings: PathMapping[];
  customMapping: boolean;
  onMappingsChange: (mappings: PathMapping[]) => void;
  onCustomMappingChange: (custom: boolean) => void;
}

function defaultHint(sourceType: VcsKind, targetType: VcsKind, branch: string) {
  if (sourceType === "svn") {
    return `默认：源仓库「${branch}」下的文件变更，对应写入目标仓库根目录。两边目录结构一致时无需修改。`;
  }
  if (targetType === "svn") {
    return "默认：源 Git 仓库中的文件变更，对应写入目标 SVN 工作副本根目录。";
  }
  return "默认：源仓库中的文件变更，对应写入目标仓库根目录。两边目录结构一致时无需修改。";
}

function customHint(sourceType: VcsKind, targetType: VcsKind) {
  const sourceLabel = sourceType === "svn" ? "SVN" : "Git";
  const targetLabel = targetType === "svn" ? "SVN" : "Git";
  return `将源 ${sourceLabel} 仓库中的路径前缀，映射到目标 ${targetLabel} 仓库中的路径前缀。`;
}

export function PathMappingPanel({
  sourceType,
  targetType,
  branch,
  mappings,
  customMapping,
  onMappingsChange,
  onCustomMappingChange,
}: Props) {
  const defaults = defaultMappingsForSource(sourceType, branch);

  const update = (index: number, field: "from" | "to", value: string) => {
    const next = [...mappings];
    next[index] = { ...next[index], [field]: value };
    onMappingsChange(next);
  };

  const toggleCustom = (checked: boolean) => {
    onCustomMappingChange(checked);
    if (checked && mappings.length === 0) {
      onMappingsChange(defaults);
    }
  };

  const fromPlaceholder = sourceType === "svn" ? "/trunk" : ".";
  const toPlaceholder = targetType === "svn" ? "/trunk" : ".";

  return (
    <div className="card path-mapping-panel">
      <h3 className="path-mapping-title">文件如何写入目标仓库</h3>
      {!customMapping ? (
        <p className="form-hint" style={{ margin: 0 }}>
          {defaultHint(sourceType, targetType, branch)}
        </p>
      ) : (
        <p className="form-hint" style={{ margin: 0 }}>
          {customHint(sourceType, targetType)}
        </p>
      )}
      <label className="form-advanced-toggle" style={{ marginTop: 12 }}>
        <input
          type="checkbox"
          checked={customMapping}
          onChange={(e) => toggleCustom(e.target.checked)}
        />
        <span>源与目标目录结构不同，需要自定义对应关系</span>
      </label>
      {customMapping && (
        <div className="form-advanced-panel">
          <div className="mapping-labels">
            <span>源仓库路径前缀</span>
            <span>目标仓库路径前缀</span>
          </div>
          {mappings.map((m, i) => (
            <div key={i} className="form-row" style={{ marginBottom: 0 }}>
              <input
                value={m.from}
                onChange={(e) => update(i, "from", e.target.value)}
                placeholder={fromPlaceholder}
              />
              <input
                value={m.to}
                onChange={(e) => update(i, "to", e.target.value)}
                placeholder={toPlaceholder}
              />
            </div>
          ))}
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => onMappingsChange([...mappings, { from: "", to: "" }])}
          >
            <IconPlus /> 添加规则
          </button>
        </div>
      )}
    </div>
  );
}
