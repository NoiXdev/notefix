import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
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

const DATE_FORMATS: { value: DateFormat; labelKey: string }[] = [
  { value: "auto", labelKey: "settings.appearance.dateFormats.auto" },
  { value: "de", labelKey: "settings.appearance.dateFormats.de" },
  { value: "iso", labelKey: "settings.appearance.dateFormats.iso" },
  { value: "us", labelKey: "settings.appearance.dateFormats.us" },
];

const PIN_SCOPES: { value: import("../hooks/useSettings").PinnedScope; labelKey: string }[] = [
  { value: "perFolder", labelKey: "settings.appearance.pinScopes.perFolder" },
  { value: "global", labelKey: "settings.appearance.pinScopes.global" },
];

const FOLDER_COLOR_STYLES: { value: import("../hooks/useSettings").FolderColorStyle; labelKey: string }[] = [
  { value: "icon", labelKey: "settings.appearance.folderColorStyles.icon" },
  { value: "bar", labelKey: "settings.appearance.folderColorStyles.bar" },
  { value: "row", labelKey: "settings.appearance.folderColorStyles.row" },
];

const START_VIEWS: { value: import("../hooks/useSettings").StartView; labelKey: string }[] = [
  { value: "lastNote", labelKey: "settings.system.startViews.lastNote" },
  { value: "dashboard", labelKey: "settings.system.startViews.dashboard" },
];

const CLOSE_ACTIONS: { value: import("../hooks/useSettings").CloseAction; labelKey: string }[] = [
  { value: "ask", labelKey: "settings.system.closeActions.ask" },
  { value: "minimize", labelKey: "settings.system.closeActions.minimize" },
  { value: "quit", labelKey: "settings.system.closeActions.quit" },
];

const LANGUAGES = [
  { value: "system", label: "" }, // Label kommt aus t() zur Laufzeit
  { value: "de", label: "Deutsch" },
  { value: "en", label: "English" },
  { value: "fr", label: "Français" },
];

export default function Settings({ onClose, settings, onSetSetting, onExport, initialPage }: Props) {
  const { t } = useTranslation();
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
          <span className="text-gray-400 text-xs font-semibold uppercase tracking-widest">{t("settings.sidebarTitle")}</span>
          <button
            onClick={onClose}
            className="w-6 h-6 flex items-center justify-center text-gray-500 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title={t("settings.backToNotes")}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
        <nav className="flex-1 py-2">
          <NavItem label={t("settings.nav.about")} active={page === "about"} onClick={() => setPage("about")} />
          <NavItem label={t("settings.nav.appearance")} active={page === "appearance"} onClick={() => setPage("appearance")} />
          <NavItem label={t("settings.nav.system")} active={page === "system"} onClick={() => setPage("system")} />
          <NavItem label={t("settings.nav.stats")} active={page === "stats"} onClick={() => setPage("stats")} />
          <NavItem label={t("settings.nav.shortcuts")} active={page === "shortcuts"} onClick={() => setPage("shortcuts")} />
          <NavItem label={t("settings.nav.diagnostics")} active={page === "diagnostics"} onClick={() => setPage("diagnostics")} />
        </nav>
      </aside>

      <main className="settings-scroll flex-1 overflow-auto px-10 py-10" style={{ background: "#fef9c3" }}>
        {page === "about" && info && (
          <div>
            <Logo size={56} className="mb-4" />
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{info.name}</h1>
            <p className="text-sm text-gray-500 mb-8">{t("settings.about.version", { version: info.version })}</p>
            <p className="text-sm text-gray-600">{info.description}</p>
          </div>
        )}

        {page === "appearance" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.appearance.title")}</h1>
            <p className="text-sm text-gray-500 mb-6">{t("settings.appearance.subtitle")}</p>
            <h2 className="text-sm font-semibold text-gray-800 mb-2">{t("settings.appearance.dateFormat")}</h2>
            <div className="max-w-sm"><Select value={settings.dateFormat} options={DATE_FORMATS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("dateFormat", v as DateFormat)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("settings.appearance.language")}</h2>
            <div className="max-w-sm"><Select value={settings.language} options={LANGUAGES.map(l => ({ value: l.value, label: l.value === "system" ? t("settings.appearance.langAuto") : l.label }))} onChange={v => onSetSetting("language", v as import("../hooks/useSettings").LangSetting)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("settings.appearance.pinned")}</h2>
            <div className="max-w-sm"><Select value={settings.pinnedScope} options={PIN_SCOPES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("pinnedScope", v as import("../hooks/useSettings").PinnedScope)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("settings.appearance.folderColor")}</h2>
            <div className="max-w-sm"><Select value={settings.folderColorStyle} options={FOLDER_COLOR_STYLES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("folderColorStyle", v as import("../hooks/useSettings").FolderColorStyle)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("settings.appearance.treeView")}</h2>
            <div className="flex flex-col gap-3 max-w-sm">
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.appearance.compactTree")}</span>
                <Toggle checked={settings.compactTree ?? false} onChange={() => onSetSetting("compactTree", !settings.compactTree)} label={t("settings.appearance.compactTree")} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.appearance.treeProgress")}</span>
                <Toggle checked={settings.treeProgress ?? true} onChange={() => onSetSetting("treeProgress", !settings.treeProgress)} label={t("settings.appearance.treeProgress")} />
              </label>
            </div>
          </div>
        )}

        {page === "system" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.system.title")}</h1>
            <p className="text-sm text-gray-500 mb-6">{t("settings.system.subtitle")}</p>
            <div className="flex flex-col gap-3 max-w-md">
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.startOnBoot")}</span>
                <Toggle checked={bootEnabled} onChange={toggleBoot} label={t("settings.system.startOnBoot")} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.startMinimized")}</span>
                <Toggle checked={settings.startMinimized} onChange={() => onSetSetting("startMinimized", !settings.startMinimized)} label={t("settings.system.startMinimized")} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.closeBehavior")}</span>
                <div className="w-56"><Select value={settings.closeAction ?? "ask"} options={CLOSE_ACTIONS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("closeAction", v as import("../hooks/useSettings").CloseAction)} /></div>
              </label>
              <button
                onClick={() => onExport([], "notefix-export.json")}
                className="mt-2 self-start px-4 py-1.5 rounded text-sm font-medium"
                style={{ background: "#fde047", color: "#1c1917" }}
              >
                {t("settings.system.exportAll")}
              </button>

              <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-1">{t("settings.system.location")}</h2>
              <p className="text-xs text-gray-600 break-all mb-2">{dbPath}</p>
              <button
                onClick={changeLocation}
                className="self-start px-4 py-1.5 rounded text-sm font-medium border"
                style={{ borderColor: "#e7d27a", color: "#1c1917" }}
              >
                {t("settings.system.change")}
              </button>
              {locResult && (
                <div className="mt-3 text-sm text-gray-700">
                  <p className="mb-2">
                    {locResult.mode === "switched"
                      ? t("settings.system.switched", { path: locResult.path })
                      : t("settings.system.moved", { path: locResult.path })}
                  </p>
                  <button
                    onClick={() => api.relaunch()}
                    className="self-start px-4 py-1.5 rounded text-sm font-medium"
                    style={{ background: "#fde047", color: "#1c1917" }}
                  >
                    {t("settings.system.restartNow")}
                  </button>
                </div>
              )}
              <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-1">{t("settings.system.editorAndHistory")}</h2>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.autosaveDelay")}</span>
                <input type="number" min={100} step={50} value={settings.autosaveDelay ?? 400} onChange={e => onSetSetting("autosaveDelay", Math.max(100, Number(e.target.value) || 400))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "#e7d27a" }} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.revisionLimit")}</span>
                <input type="number" min={1} value={settings.revisionLimit ?? 50} onChange={e => onSetSetting("revisionLimit", Math.max(1, Number(e.target.value) || 50))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "#e7d27a" }} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.startView")}</span>
                <div className="w-56"><Select value={settings.startView ?? "lastNote"} options={START_VIEWS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("startView", v as import("../hooks/useSettings").StartView)} /></div>
              </label>
              <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-1">{t("settings.system.trash")}</h2>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.trashEnabled")}</span>
                <Toggle checked={settings.trashEnabled ?? true} onChange={() => onSetSetting("trashEnabled", !settings.trashEnabled)} label={t("settings.system.trashEnabled")} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.trashRetention")}</span>
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
            <h1 className="text-2xl font-bold text-gray-900 mb-6">{t("settings.stats.title")}</h1>
            {stats && (
              <dl className="grid grid-cols-2 gap-4 max-w-md text-gray-800">
                <div><dt className="text-xs text-gray-500">{t("settings.stats.notes")}</dt><dd className="text-2xl font-bold">{stats.notes}</dd></div>
                <div><dt className="text-xs text-gray-500">{t("settings.stats.archived")}</dt><dd className="text-2xl font-bold">{stats.archived}</dd></div>
                <div><dt className="text-xs text-gray-500">{t("settings.stats.characters")}</dt><dd className="text-2xl font-bold">{stats.characters}</dd></div>
                <div><dt className="text-xs text-gray-500">{t("settings.stats.words")}</dt><dd className="text-2xl font-bold">{stats.words}</dd></div>
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
  const { t } = useTranslation();
  const [checks, setChecks] = useState<import("../systemChecks").SystemCheck[] | null>(null);
  const run = () => { void runSystemChecks(settings).then(setChecks); };
  useEffect(run, [settings]);
  const color = (s: string) => s === 'ok' ? '#16a34a' : s === 'warn' ? '#d97706' : '#dc2626';
  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("diagnostics.title")}</h1>
      <p className="text-sm text-gray-500 mb-6">{t("diagnostics.subtitle")}</p>
      <div className="flex flex-col gap-3 max-w-lg">
        {(checks ?? []).map(c => (
          <div key={c.key} className="flex items-start justify-between gap-3 text-sm">
            <div>
              <div className="font-medium text-gray-800"><span style={{ color: color(c.status) }}>●</span> {c.label}</div>
              <div className="text-xs text-gray-500 break-all">{c.detail}</div>
            </div>
            {c.action === 'changeLocation' && (
              <button onClick={onChangeLocation} className="shrink-0 px-3 py-1 rounded text-xs font-medium border" style={{ borderColor: "#e7d27a", color: "#1c1917" }}>{t("diagnostics.changeLocation")}</button>
            )}
          </div>
        ))}
        <button onClick={run} className="self-start mt-2 px-4 py-1.5 rounded text-sm font-medium border" style={{ borderColor: "#e7d27a", color: "#1c1917" }}>{t("diagnostics.recheck")}</button>
      </div>
    </div>
  );
}
