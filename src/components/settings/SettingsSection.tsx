import type { ReactNode } from "react";

interface Props {
  title: string;
  children: ReactNode;
}

/** A quiet card grouping related settings rows, with a small muted title. */
export default function SettingsSection({ title, children }: Props) {
  return (
    <section
      className="rounded-xl p-4 min-w-0"
      style={{ background: "color-mix(in srgb, var(--paper), #fff 45%)", border: "1px solid var(--line)" }}
    >
      <h3 className="text-xs font-semibold mb-3" style={{ color: "var(--ink-muted)" }}>
        {title}
      </h3>
      <div className="flex flex-col gap-3">{children}</div>
    </section>
  );
}
