import type { ReactNode } from "react";

interface Props {
  label: string;
  hint?: string;
  /** Put the control below the label, full width, instead of beside it. */
  stack?: boolean;
  children: ReactNode;
}

/** One full-width setting row: label (+ optional hint) left, control right. */
export default function SettingRow({ label, hint, stack, children }: Props) {
  return (
    <div className={stack ? "flex flex-col gap-2" : "flex items-center justify-between gap-4"}>
      <div className="min-w-0">
        <div className="text-sm" style={{ color: "var(--ink)" }}>{label}</div>
        {hint && <div className="text-xs mt-0.5" style={{ color: "var(--ink-muted)" }}>{hint}</div>}
      </div>
      <div className={stack ? "w-full" : "shrink-0"}>{children}</div>
    </div>
  );
}
