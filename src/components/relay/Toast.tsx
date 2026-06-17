import { useEffect } from "react";
import type { ReactNode } from "react";

export function Toast({ message, onDone }: { message: ReactNode; onDone: () => void }) {
  useEffect(() => {
    const t = setTimeout(onDone, 3200);
    return () => clearTimeout(t);
  }, [onDone]);

  return <div className="toast">{message}</div>;
}
