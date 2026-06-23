import { useEffect, useState } from "react";
import { api, type AppInfo } from "../api";
import type { Stats } from "../types";
import type { DateFormat } from "../dates";
import type { AppSettings, PinnedDisplayMode } from "../hooks/useSettings";
import { exportSelected } from "../export";

type Page = "about" | "appearance" | "system" | "stats";

interface NavItemProps {
  label: string;
  active: boolean;
  onClick: () => void;
}

function NavItem({ label, active, onClick }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={`w-full text-left px-4 py-2 text-sm transition-colors ${
        active ? "bg-gray-800 text-white" : "text-gray-400 hover:bg-gray-900 hover:text-gray-200"
      }`}
    >
      {label}
    </button>
  );
}

interface Props {
  onClose: () => void;
  settings: AppSettings;
  onSetSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void;
}

const MODES: { value: PinnedDisplayMode; label: string }[] = [
  { value: "flat", label: "Flach (Pin-Icon + Trennlinie)" },
  { value: "sections", label: "Sektionen (Überschriften)" },
];

const DATE_FORMATS: { value: DateFormat; label: string }[] = [
  { value: "auto", label: "Auto (relativ)" },
  { value: "de", label: "TT.MM.JJJJ" },
  { value: "iso", label: "JJJJ-MM-TT" },
  { value: "us", label: "MM/TT/JJJJ" },
];

const PIN_SCOPES: { value: import("../hooks/useSettings").PinnedScope; label: string }[] = [
  { value: "perFolder", label: "Gepinnt zuerst je Ordner" },
  { value: "global", label: "Globale „Angepinnt“-Sektion" },
];

export default function Settings({ onClose, settings, onSetSetting }: Props) {
  const [page, setPage] = useState<Page>("about");
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    api.getAppInfo().then(setInfo);
  }, []);

  const [bootEnabled, setBootEnabled] = useState(false);
  useEffect(() => {
    api.autostart.isEnabled().then(setBootEnabled);
  }, []);

  const [stats, setStats] = useState<Stats | null>(null);
  useEffect(() => {
    api.stats().then(setStats);
  }, []);

  const toggleBoot = async () => {
    const next = !bootEnabled;
    setBootEnabled(next);
    if (next) await api.autostart.enable();
    else await api.autostart.disable();
  };

  return (
    <div className="flex h-screen overflow-hidden">
      <aside className="w-52 shrink-0 bg-gray-950 flex flex-col h-full select-none">
        <div className="px-4 py-3 flex items-center justify-between border-b border-gray-800">
          <span className="text-gray-400 text-xs font-semibold uppercase tracking-widest">Settings</span>
          <button
            onClick={onClose}
            className="w-6 h-6 flex items-center justify-center text-gray-500 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title="Back to notes"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
        <nav className="flex-1 py-2">
          <NavItem label="About" active={page === "about"} onClick={() => setPage("about")} />
          <NavItem label="Darstellung" active={page === "appearance"} onClick={() => setPage("appearance")} />
          <NavItem label="System" active={page === "system"} onClick={() => setPage("system")} />
          <NavItem label="Statistik" active={page === "stats"} onClick={() => setPage("stats")} />
        </nav>
      </aside>

      <main className="flex-1 overflow-auto px-10 py-10" style={{ background: "#fef9c3" }}>
        {page === "about" && info && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{info.name}</h1>
            <p className="text-sm text-gray-500 mb-8">Version {info.version}</p>
            <p className="text-sm text-gray-600">{info.description}</p>
          </div>
        )}

        {page === "appearance" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">Darstellung</h1>
            <p className="text-sm text-gray-500 mb-6">Wie angepinnte Notizen in der Liste erscheinen.</p>
            <div className="flex flex-col gap-2 max-w-sm">
              {MODES.map(mode => {
                const active = settings.pinnedDisplayMode === mode.value;
                return (
                  <button
                    key={mode.value}
                    onClick={() => onSetSetting("pinnedDisplayMode", mode.value)}
                    className="text-left px-4 py-2.5 rounded text-sm transition-colors border"
                    style={{
                      background: active ? "#fde047" : "transparent",
                      borderColor: active ? "#eab308" : "#e7d27a",
                      color: "#1c1917",
                    }}
                  >
                    {mode.label}
                  </button>
                );
              })}
            </div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">Datumsformat</h2>
            <div className="flex flex-col gap-2 max-w-sm">
              {DATE_FORMATS.map(f => {
                const active = settings.dateFormat === f.value;
                return (
                  <button
                    key={f.value}
                    onClick={() => onSetSetting("dateFormat", f.value)}
                    className="text-left px-4 py-2.5 rounded text-sm transition-colors border"
                    style={{ background: active ? "#fde047" : "transparent", borderColor: active ? "#eab308" : "#e7d27a", color: "#1c1917" }}
                  >
                    {f.label}
                  </button>
                );
              })}
            </div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">Angepinnte Notizen</h2>
            <div className="flex flex-col gap-2 max-w-sm">
              {PIN_SCOPES.map(s => {
                const active = settings.pinnedScope === s.value;
                return (
                  <button key={s.value} onClick={() => onSetSetting("pinnedScope", s.value)} className="text-left px-4 py-2.5 rounded text-sm transition-colors border" style={{ background: active ? "#fde047" : "transparent", borderColor: active ? "#eab308" : "#e7d27a", color: "#1c1917" }}>
                    {s.label}
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {page === "system" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">System</h1>
            <p className="text-sm text-gray-500 mb-6">Start- und Hintergrund-Verhalten.</p>
            <div className="flex flex-col gap-3 max-w-md">
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Bei Anmeldung starten</span>
                <input type="checkbox" checked={bootEnabled} onChange={toggleBoot} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Minimiert starten (nur Menüleiste)</span>
                <input
                  type="checkbox"
                  checked={settings.startMinimized}
                  onChange={() => onSetSetting("startMinimized", !settings.startMinimized)}
                />
              </label>
              <button
                onClick={() => exportSelected([], "notefix-export.json")}
                className="mt-2 self-start px-4 py-1.5 rounded text-sm font-medium"
                style={{ background: "#fde047", color: "#1c1917" }}
              >
                Alle als JSON exportieren
              </button>
            </div>
          </div>
        )}

        {page === "stats" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-6">Statistik</h1>
            {stats && (
              <dl className="grid grid-cols-2 gap-4 max-w-md text-gray-800">
                <div><dt className="text-xs text-gray-500">Notizen</dt><dd className="text-2xl font-bold">{stats.notes}</dd></div>
                <div><dt className="text-xs text-gray-500">Archiviert</dt><dd className="text-2xl font-bold">{stats.archived}</dd></div>
                <div><dt className="text-xs text-gray-500">Zeichen</dt><dd className="text-2xl font-bold">{stats.characters}</dd></div>
                <div><dt className="text-xs text-gray-500">Wörter</dt><dd className="text-2xl font-bold">{stats.words}</dd></div>
              </dl>
            )}
          </div>
        )}
      </main>
    </div>
  );
}
