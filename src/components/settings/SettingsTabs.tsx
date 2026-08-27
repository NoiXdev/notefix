export interface SettingsTab {
  id: string;
  label: string;
}

interface Props {
  tabs: SettingsTab[];
  active: string;
  onChange: (id: string) => void;
}

/** Horizontal underline tab bar used to split a settings page into sub-areas. */
export default function SettingsTabs({ tabs, active, onChange }: Props) {
  return (
    <div className="flex items-center gap-1 mb-6 border-b overflow-x-auto" style={{ borderColor: "var(--line)" }}>
      {tabs.map(tab => {
        const isActive = tab.id === active;
        return (
          <button
            key={tab.id}
            type="button"
            onClick={() => onChange(tab.id)}
            className="shrink-0 px-3 py-2 -mb-px text-sm font-medium border-b-2 transition-colors"
            style={{
              borderColor: isActive ? "var(--accent-strong)" : "transparent",
              color: isActive ? "var(--ink)" : "var(--ink-muted)",
            }}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
