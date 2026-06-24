import { useEffect, useState } from "react";
import { api, type AppInfo } from "../api";
import type { Stats } from "../types";
import type { DateFormat } from "../dates";
import type { AppSettings } from "../hooks/useSettings";
import Logo from "./Logo";
import Select from "./Select";
import Toggle from "./Toggle";
import ShortcutsSettings from "./ShortcutsSettings";
import { runSystemChecks } from "../systemChecks";

type Page = "about" | "appearance" | "system" | "stats" | "shortcuts" | "diagnostics";

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
  onExport: (ids: string[], name: string) => void;
  initialPage?: Page;
}

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

const FOLDER_COLOR_STYLES: { value: import("../hooks/useSettings").FolderColorStyle; label: string }[] = [
  { value: "icon", label: "Nur Icon einfärben" },
  { value: "bar", label: "Icon + Akzentbalken" },
  { value: "row", label: "Ganze Zeile tönen" },
];

const START_VIEWS: { value: import("../hooks/useSettings").StartView; label: string }[] = [
  { value: "lastNote", label: "Zuletzt geöffnete Notiz" },
  { value: "dashboard", label: "Dashboard" },
];

const CLOSE_ACTIONS: { value: import("../hooks/useSettings").CloseAction; label: string }[] = [
  { value: "ask", label: "Fragen" },
  { value: "minimize", label: "In Menüleiste minimieren" },
  { value: "quit", label: "Beenden" },
];

export default function Settings({ onClose, settings, onSetSetting, onExport, initialPage }: Props) {
  const [page, setPage] = useState<Page>(initialPage ?? "about");
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

  const [dbPath, setDbPath] = useState("");
  const [locResult, setLocResult] = useState<{ mode: string; path: string } | null>(null);
  useEffect(() => {
    api.getDbPath().then(setDbPath);
  }, []);
  const changeLocation = async () => {
    const folder = await api.pickFolder();
    if (!folder) return;
    const res = await api.setDbLocation(folder);
    setLocResult(res);
    setDbPath(res.path);
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
          <NavItem label="Tastatur" active={page === "shortcuts"} onClick={() => setPage("shortcuts")} />
          <NavItem label="Diagnose" active={page === "diagnostics"} onClick={() => setPage("diagnostics")} />
        </nav>
      </aside>

      <main className="settings-scroll flex-1 overflow-auto px-10 py-10" style={{ background: "#fef9c3" }}>
        {page === "about" && info && (
          <div>
            <Logo size={56} className="mb-4" />
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{info.name}</h1>
            <p className="text-sm text-gray-500 mb-8">Version {info.version}</p>
            <p className="text-sm text-gray-600">{info.description}</p>
          </div>
        )}

        {page === "appearance" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">Darstellung</h1>
            <p className="text-sm text-gray-500 mb-6">Wie angepinnte Notizen in der Liste erscheinen.</p>
            <h2 className="text-sm font-semibold text-gray-800 mb-2">Datumsformat</h2>
            <div className="max-w-sm"><Select value={settings.dateFormat} options={DATE_FORMATS} onChange={v => onSetSetting("dateFormat", v as DateFormat)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">Angepinnte Notizen</h2>
            <div className="max-w-sm"><Select value={settings.pinnedScope} options={PIN_SCOPES} onChange={v => onSetSetting("pinnedScope", v as import("../hooks/useSettings").PinnedScope)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">Ordnerfarbe</h2>
            <div className="max-w-sm"><Select value={settings.folderColorStyle} options={FOLDER_COLOR_STYLES} onChange={v => onSetSetting("folderColorStyle", v as import("../hooks/useSettings").FolderColorStyle)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">Baum-Ansicht</h2>
            <div className="flex flex-col gap-3 max-w-sm">
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Kompakte Ansicht (nur Titel)</span>
                <Toggle checked={settings.compactTree ?? false} onChange={() => onSetSetting("compactTree", !settings.compactTree)} label="Kompakte Ansicht (nur Titel)" />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Fortschritt im Baum zeigen</span>
                <Toggle checked={settings.treeProgress ?? true} onChange={() => onSetSetting("treeProgress", !settings.treeProgress)} label="Fortschritt im Baum zeigen" />
              </label>
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
                <Toggle checked={bootEnabled} onChange={toggleBoot} label="Bei Anmeldung starten" />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Minimiert starten (nur Menüleiste)</span>
                <Toggle checked={settings.startMinimized} onChange={() => onSetSetting("startMinimized", !settings.startMinimized)} label="Minimiert starten (nur Menüleiste)" />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Beim Schließen des Fensters</span>
                <div className="w-56"><Select value={settings.closeAction ?? "ask"} options={CLOSE_ACTIONS} onChange={v => onSetSetting("closeAction", v as import("../hooks/useSettings").CloseAction)} /></div>
              </label>
              <button
                onClick={() => onExport([], "notefix-export.json")}
                className="mt-2 self-start px-4 py-1.5 rounded text-sm font-medium"
                style={{ background: "#fde047", color: "#1c1917" }}
              >
                Alle als JSON exportieren
              </button>

              <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-1">Speicherort</h2>
              <p className="text-xs text-gray-600 break-all mb-2">{dbPath}</p>
              <button
                onClick={changeLocation}
                className="self-start px-4 py-1.5 rounded text-sm font-medium border"
                style={{ borderColor: "#e7d27a", color: "#1c1917" }}
              >
                Ändern…
              </button>
              {locResult && (
                <div className="mt-3 text-sm text-gray-700">
                  <p className="mb-2">
                    {locResult.mode === "switched"
                      ? `Gewechselt zur vorhandenen DB unter ${locResult.path}. Deine bisherigen Notizen bleiben am alten Ort.`
                      : `Verschoben nach ${locResult.path}.`}
                  </p>
                  <button
                    onClick={() => api.relaunch()}
                    className="self-start px-4 py-1.5 rounded text-sm font-medium"
                    style={{ background: "#fde047", color: "#1c1917" }}
                  >
                    Jetzt neu starten
                  </button>
                </div>
              )}
              <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-1">Editor &amp; Verlauf</h2>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Auto-Save-Verzögerung (ms)</span>
                <input type="number" min={100} step={50} value={settings.autosaveDelay ?? 400} onChange={e => onSetSetting("autosaveDelay", Math.max(100, Number(e.target.value) || 400))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "#e7d27a" }} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Versionen pro Notiz</span>
                <input type="number" min={1} value={settings.revisionLimit ?? 50} onChange={e => onSetSetting("revisionLimit", Math.max(1, Number(e.target.value) || 50))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "#e7d27a" }} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Startansicht</span>
                <div className="w-56"><Select value={settings.startView ?? "lastNote"} options={START_VIEWS} onChange={v => onSetSetting("startView", v as import("../hooks/useSettings").StartView)} /></div>
              </label>
              <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-1">Papierkorb</h2>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Papierkorb verwenden</span>
                <Toggle checked={settings.trashEnabled ?? true} onChange={() => onSetSetting("trashEnabled", !settings.trashEnabled)} label="Papierkorb verwenden" />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>Automatisch leeren nach (Tagen)</span>
                <input type="number" min={1} value={settings.trashRetentionDays ?? 30} onChange={e => onSetSetting("trashRetentionDays", Math.max(1, Number(e.target.value) || 30))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "#e7d27a" }} />
              </label>
            </div>
          </div>
        )}

        {page === "shortcuts" && (
          <ShortcutsSettings value={settings.shortcuts} onChange={v => onSetSetting("shortcuts", v)} />
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

        {page === "diagnostics" && (
          <SystemChecksPage settings={settings} onChangeLocation={changeLocation} />
        )}
      </main>
    </div>
  );
}

function SystemChecksPage({ settings, onChangeLocation }: { settings: AppSettings; onChangeLocation: () => void }) {
  const [checks, setChecks] = useState<import("../systemChecks").SystemCheck[] | null>(null);
  const run = () => { void runSystemChecks(settings).then(setChecks); };
  useEffect(run, [settings]);
  const color = (s: string) => s === 'ok' ? '#16a34a' : s === 'warn' ? '#d97706' : '#dc2626';
  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">Diagnose</h1>
      <p className="text-sm text-gray-500 mb-6">Prüft die Rechte und Ordner, die Notefix zum Arbeiten braucht.</p>
      <div className="flex flex-col gap-3 max-w-lg">
        {(checks ?? []).map(c => (
          <div key={c.key} className="flex items-start justify-between gap-3 text-sm">
            <div>
              <div className="font-medium text-gray-800"><span style={{ color: color(c.status) }}>●</span> {c.label}</div>
              <div className="text-xs text-gray-500 break-all">{c.detail}</div>
            </div>
            {c.action === 'changeLocation' && (
              <button onClick={onChangeLocation} className="shrink-0 px-3 py-1 rounded text-xs font-medium border" style={{ borderColor: "#e7d27a", color: "#1c1917" }}>Speicherort ändern…</button>
            )}
          </div>
        ))}
        <button onClick={run} className="self-start mt-2 px-4 py-1.5 rounded text-sm font-medium border" style={{ borderColor: "#e7d27a", color: "#1c1917" }}>Erneut prüfen</button>
      </div>
    </div>
  );
}
