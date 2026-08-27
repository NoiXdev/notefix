import type { ReactNode } from "react";

/** Responsive grid of SettingsSection cards: 2 columns on desktop, 1 on mobile/narrow. */
export default function SettingsGrid({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-1 md:grid-cols-2 gap-3">{children}</div>;
}
