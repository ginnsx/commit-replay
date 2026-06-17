import type { PathMapping } from "../../lib/types";
import { defaultSvnMappings } from "../../lib/constants";
import { IconPlus } from "./icons";

interface Props {
  branch: string;
  mappings: PathMapping[];
  customMapping: boolean;
  onMappingsChange: (mappings: PathMapping[]) => void;
  onCustomMappingChange: (custom: boolean) => void;
}

export function PathMappingPanel({
  branch,
  mappings,
  customMapping,
  onMappingsChange,
  onCustomMappingChange,
}: Props) {
  const update = (index: number, field: "from" | "to", value: string) => {
    const next = [...mappings];
    next[index] = { ...next[index], [field]: value };
    onMappingsChange(next);
  };

  const toggleCustom = (checked: boolean) => {
    onCustomMappingChange(checked);
    if (!checked) {
      onMappingsChange(defaultSvnMappings(branch));
    } else if (mappings.length === 0) {
      onMappingsChange(defaultSvnMappings(branch));
    }
  };

  return (
    <div className="card path-mapping-panel">
      <h3 className="path-mapping-title">文件如何写入目标仓库</h3>
      {!customMapping ? (
        <p className="form-hint" style={{ margin: 0 }}>
          默认：源仓库「{branch}」下的文件变更，对应写入目标仓库根目录。两边目录结构一致时无需修改。
        </p>
      ) : (
        <p className="form-hint" style={{ margin: 0 }}>
          将 SVN 提交中的路径前缀，映射到目标 Git 仓库中的路径前缀。
        </p>
      )}
      <label className="form-advanced-toggle" style={{ marginTop: 12 }}>
        <input type="checkbox" checked={customMapping} onChange={(e) => toggleCustom(e.target.checked)} />
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
                placeholder="/trunk"
              />
              <input
                value={m.to}
                onChange={(e) => update(i, "to", e.target.value)}
                placeholder="."
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
