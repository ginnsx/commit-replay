import type { FileChange, PreviewUnit } from "../lib/types";

export type FileTreeMode = "aggregated" | "by_unit";

interface FileTreeProps {
  mode: FileTreeMode;
  aggregated: FileChange[];
  units: PreviewUnit[];
  selectedPath: string | null;
  onSelect: (path: string) => void;
}

function riskClass(risk: FileChange["conflict_risk"]): string {
  return risk === "high" ? "text-orange-400" : "text-gray-300";
}

export default function FileTree({
  mode,
  aggregated,
  units,
  selectedPath,
  onSelect,
}: FileTreeProps) {
  if (mode === "aggregated") {
    return (
      <ul className="overflow-y-auto text-sm">
        {aggregated.map((f) => {
          const path = f.target_path ?? f.path;
          const active = selectedPath === path;
          return (
            <li key={path}>
              <button
                type="button"
                className={`w-full truncate px-3 py-1.5 text-left hover:bg-gray-800 ${
                  active ? "bg-gray-800 text-blue-300" : riskClass(f.conflict_risk)
                }`}
                title={path}
                onClick={() => onSelect(path)}
              >
                {path}
              </button>
            </li>
          );
        })}
      </ul>
    );
  }

  return (
    <div className="overflow-y-auto text-sm">
      {units.map((unit) => (
        <div key={unit.meta.source_ref} className="mb-2">
          <div className="px-3 py-1 text-xs font-medium text-gray-500">{unit.meta.source_ref}</div>
          <ul>
            {unit.files.map((f) => {
              const path = f.target_path ?? f.path;
              const active = selectedPath === path;
              return (
                <li key={`${unit.meta.source_ref}-${path}`}>
                  <button
                    type="button"
                    className={`w-full truncate px-3 py-1.5 text-left hover:bg-gray-800 ${
                      active ? "bg-gray-800 text-blue-300" : riskClass(f.conflict_risk)
                    }`}
                    title={path}
                    onClick={() => onSelect(path)}
                  >
                    {path}
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </div>
  );
}
