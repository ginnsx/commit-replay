import type { ReactNode } from "react";

interface Props {
  left: ReactNode;
  children?: ReactNode;
  showBack?: boolean;
  onBack?: () => void;
  onNext?: () => void;
  nextLabel?: ReactNode;
  nextDisabled?: boolean;
  primaryAction?: ReactNode;
}

export function BottomBar({
  left,
  children,
  showBack = true,
  onBack,
  onNext,
  nextLabel = "下一步",
  nextDisabled,
  primaryAction,
}: Props) {
  return (
    <div className="main-footer">
      <span className="footer-left">{left}</span>
      <div className="footer-actions">
        {showBack && onBack && (
          <button type="button" className="btn btn-ghost" onClick={onBack}>
            上一步
          </button>
        )}
        {primaryAction ??
          (onNext && (
            <button type="button" className="btn btn-primary" disabled={nextDisabled} onClick={onNext}>
              {nextLabel}
            </button>
          ))}
        {children}
      </div>
    </div>
  );
}
