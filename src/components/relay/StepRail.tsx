import type { WizardStep } from "../../lib/types";
import { STEPS } from "../../lib/constants";
import { IconCheck, IconSettings } from "./icons";

interface Props {
  current: WizardStep | null;
  completed: Set<WizardStep>;
  showRepos: boolean;
  onStep: (id: WizardStep) => void;
  onManageRepos: () => void;
}

export function StepRail({ current, completed, showRepos, onStep, onManageRepos }: Props) {
  const stepIndex = current ? STEPS.findIndex((s) => s.id === current) : -1;

  return (
    <nav className="step-rail">
      <div className="step-rail-header">迁移流程</div>
      <div className="step-list">
        {STEPS.map((step, i) => {
          const done = completed.has(step.id);
          const active = step.id === current;
          const reachable = i <= stepIndex || done;
          return (
            <button
              key={step.id}
              type="button"
              className={`step-item${active ? " active" : ""}${done ? " done" : ""}`}
              disabled={!reachable && !showRepos}
              onClick={() => reachable && onStep(step.id)}
            >
              <span className="step-num">{done ? <IconCheck /> : step.num}</span>
              <span className="step-label">{step.label}</span>
            </button>
          );
        })}
      </div>
      <div className="rail-footer">
        <button
          type="button"
          className={`rail-footer-btn${showRepos ? " active" : ""}`}
          onClick={onManageRepos}
        >
          <IconSettings />
          设置
        </button>
      </div>
    </nav>
  );
}
