import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faGlobe } from "@fortawesome/free-solid-svg-icons";
import { api, type AppInfo, type UpdateInfo } from "../api";
import type { ContextInfo } from "../contexts";
import { startServerAuth } from "../serverAuth";
import type { Stats } from "../types";
import type { DateFormat } from "../dates";
import type { AppSettings } from "../hooks/useSettings";
import Logo from "./Logo";
import Select from "./Select";
import Toggle from "./Toggle";
import ShortcutsSettings from "./ShortcutsSettings";
import PromptDialog from "./PromptDialog";
import { runSystemChecks } from "../systemChecks";
import { OSS_LIBS } from "../licenses";

export type Page = "about" | "appearance" | "system" | "contexts" | "mcp" | "stats" | "shortcuts" | "diagnostics";

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

function UpdateChecker({ settings, onSetSetting }: {
  settings: AppSettings;
  onSetSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void;
}) {
  const { t } = useTranslation();
  const [state, setState] = useState<"idle" | "checking" | "error" | UpdateInfo>("idle");
  const check = () => {
    setState("checking");
    api.checkForUpdate().then(setState).catch(() => setState("error"));
  };
  return (
    <div className="mt-10 max-w-md">
      <h2 className="text-sm font-semibold text-gray-800 mb-2">{t("update.title")}</h2>
      <div className="flex items-center gap-3 flex-wrap">
        <button
          onClick={check}
          disabled={state === "checking"}
          className="px-3 py-1.5 text-sm rounded bg-gray-800 text-white hover:bg-gray-700 disabled:opacity-50"
        >
          {state === "checking" ? t("update.checking") : t("update.check")}
        </button>
        {typeof state === "object" && (state.updateAvailable ? (
          <button onClick={() => void api.openExternal(state.url)} className="text-sm text-blue-700 underline">
            {t("update.available", { version: state.latest })}
          </button>
        ) : (
          <span className="text-sm text-gray-600">{t("update.upToDate", { version: state.current })}</span>
        ))}
        {state === "error" && <span className="text-sm text-red-600">{t("update.error")}</span>}
      </div>
      <label className="mt-4 flex items-center justify-between gap-4 text-sm text-gray-800 max-w-sm">
        <span>{t("update.onStart")}</span>
        <Toggle
          checked={settings.checkUpdatesOnStart}
          onChange={() => onSetSetting("checkUpdatesOnStart", !settings.checkUpdatesOnStart)}
          label={t("update.onStart")}
        />
      </label>
    </div>
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

const LINK_PREVIEW_MODES: { value: "url" | "inline" | "card"; labelKey: string }[] = [
  { value: "url", labelKey: "settings.appearance.linkPreviewModes.url" },
  { value: "inline", labelKey: "settings.appearance.linkPreviewModes.inline" },
  { value: "card", labelKey: "settings.appearance.linkPreviewModes.card" },
];

const COPY_FORMATS: { value: import("../copyFormat").CopyFormat; labelKey: string }[] = [
  { value: "richtext", labelKey: "settings.appearance.copyFormats.richtext" },
  { value: "html", labelKey: "settings.appearance.copyFormats.html" },
  { value: "md", labelKey: "settings.appearance.copyFormats.md" },
  { value: "text", labelKey: "settings.appearance.copyFormats.text" },
];

const START_VIEWS: { value: import("../hooks/useSettings").StartView; labelKey: string }[] = [
  { value: "lastNote", labelKey: "settings.system.startViews.lastNote" },
  { value: "dashboard", labelKey: "settings.system.startViews.dashboard" },
];

const SIDEBAR_MODES: { value: import("../hooks/useSettings").SidebarMode; labelKey: string }[] = [
  { value: "switcher", labelKey: "settings.appearance.sidebarModes.switcher" },
  { value: "combined", labelKey: "settings.appearance.sidebarModes.combined" },
];

const CLOSE_ACTIONS: { value: import("../hooks/useSettings").CloseAction; labelKey: string }[] = [
  { value: "ask", labelKey: "settings.system.closeActions.ask" },
  { value: "minimize", labelKey: "settings.system.closeActions.minimize" },
  { value: "quit", labelKey: "settings.system.closeActions.quit" },
];

const MCP_BINDS: { value: "internal" | "external"; labelKey: string }[] = [
  { value: "internal", labelKey: "settings.mcp.binds.internal" },
  { value: "external", labelKey: "settings.mcp.binds.external" },
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
          <NavItem label={t("contexts.nav")} active={page === "contexts"} onClick={() => setPage("contexts")} />
          <NavItem label={t("settings.nav.mcp")} active={page === "mcp"} onClick={() => setPage("mcp")} />
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
            <p className="text-sm text-gray-600 mt-4 max-w-md">{t("settings.about.story")}</p>
            <div className="mt-6 flex flex-col gap-1 text-sm">
              <a href="https://noix.dev" className="text-blue-700 underline">{t("settings.about.project")}</a>
              <a href="https://docs.noix.dev" className="text-blue-700 underline">{t("settings.about.docs")}</a>
              <span className="text-gray-500 mt-2">{t("settings.about.license")}</span>
            </div>

            <UpdateChecker settings={settings} onSetSetting={onSetSetting} />

            <div className="mt-10 max-w-md">
              <h2 className="text-sm font-semibold text-gray-800 mb-1">{t("settings.about.openSource")}</h2>
              <p className="text-xs text-gray-500 mb-3">{t("settings.about.openSourceIntro")}</p>
              <ul className="flex flex-col divide-y divide-yellow-200/70 border-y border-yellow-200/70">
                {OSS_LIBS.map(lib => (
                  <li key={lib.name} className="flex items-center justify-between gap-3 py-1.5 text-xs">
                    <a href={lib.url} className="text-blue-700 underline">{lib.name}</a>
                    <span className="text-gray-500 whitespace-nowrap">{lib.license}</span>
                  </li>
                ))}
              </ul>
            </div>
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

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("settings.appearance.sidebarMode")}</h2>
            <div className="max-w-sm"><Select value={settings.sidebarMode} options={SIDEBAR_MODES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("sidebarMode", v as import("../hooks/useSettings").SidebarMode)} /></div>

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

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("settings.appearance.linkPreview")}</h2>
            <div className="flex flex-col gap-3 max-w-sm">
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.appearance.linkPreview")}</span>
                <Toggle checked={settings.linkPreviewEnabled ?? true} onChange={() => onSetSetting("linkPreviewEnabled", !settings.linkPreviewEnabled)} label={t("settings.appearance.linkPreview")} />
              </label>
              <Select value={settings.linkPreviewMode ?? "card"} options={LINK_PREVIEW_MODES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("linkPreviewMode", v as "url" | "inline" | "card")} />
            </div>

            <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("settings.appearance.copyFormat")}</h2>
            <div className="max-w-sm"><Select value={settings.copyFormat ?? "md"} options={COPY_FORMATS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("copyFormat", v as import("../copyFormat").CopyFormat)} /></div>
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

        {page === "contexts" && (
          <ContextsPage />
        )}

        {page === "mcp" && (
          <McpPage settings={settings} onSetSetting={onSetSetting} />
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

type CtxDialog =
  | { mode: "add" }
  | { mode: "addServer" }
  | { mode: "rename"; c: ContextInfo }
  | { mode: "remove"; c: ContextInfo }
  | null;

function ContextsPage() {
  const { t } = useTranslation();
  const [ctx, setCtx] = useState<ContextInfo[]>([]);
  const [dialog, setDialog] = useState<CtxDialog>(null);
  const [deleteFile, setDeleteFile] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void api.contexts.list().then(setCtx);
    // A completed server auth emits context-changed; refresh + clear pending.
    return api.onContextChanged(() => {
      setConnecting(false);
      void api.contexts.list().then(setCtx);
    });
  }, []);

  const labelOf = (c: ContextInfo) =>
    c.label || (c.kind === "server" ? c.serverUrl : t("contexts.localDefault"));
  const close = () => { setDialog(null); setDeleteFile(false); };

  const submitServer = async (raw: string) => {
    close();
    const urlStr = raw.trim();
    if (!urlStr) return;
    setError(null);
    setConnecting(true);
    try {
      await startServerAuth(urlStr);
    } catch {
      setConnecting(false);
      setError(t("contexts.serverError"));
    }
  };

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("contexts.title")}</h1>
      <p className="text-sm text-gray-500 mb-6">{t("contexts.subtitle")}</p>
      <div className="flex flex-col gap-2 max-w-lg">
        {ctx.map(c => (
          <div key={c.id} className="flex items-start justify-between gap-3 rounded border px-3 py-2" style={{ borderColor: "#e7d27a", background: "#fffdf0" }}>
            <div className="min-w-0">
              <div className="flex items-center gap-2 text-sm font-medium text-gray-800">
                {c.kind === "server" && <FontAwesomeIcon icon={faGlobe} className="text-[11px] text-gray-500 shrink-0" />}
                <span className="truncate">{labelOf(c)}</span>
                {c.active && (
                  <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide" style={{ background: "#fde047", color: "#1c1917" }}>{t("contexts.active")}</span>
                )}
              </div>
              <div className="text-xs text-gray-500 break-all font-mono">{c.kind === "server" ? c.serverUrl : c.path}</div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <button onClick={() => setDialog({ mode: "rename", c })} className="px-3 py-1 rounded text-xs font-medium border" style={{ borderColor: "#e7d27a", color: "#1c1917" }}>{t("contexts.rename")}</button>
              <button onClick={() => setDialog({ mode: "remove", c })} disabled={c.active} className="px-3 py-1 rounded text-xs font-medium border disabled:opacity-40 disabled:cursor-not-allowed" style={{ borderColor: "#e7d27a", color: "#1c1917" }}>{t("contexts.remove")}</button>
            </div>
          </div>
        ))}
      </div>
      <div className="mt-4 flex items-center gap-2">
        <button onClick={() => setDialog({ mode: "add" })} className="px-4 py-1.5 rounded text-sm font-medium" style={{ background: "#fde047", color: "#1c1917" }}>{t("contexts.add")}</button>
        <button onClick={() => { setError(null); setDialog({ mode: "addServer" }); }} className="px-4 py-1.5 rounded text-sm font-medium border" style={{ borderColor: "#e7d27a", color: "#1c1917" }}>{t("contexts.addServer")}</button>
        {connecting && <span className="text-xs text-gray-500">{t("contexts.connecting")}</span>}
        {error && <span className="text-xs text-red-600" role="alert">{error}</span>}
      </div>

      {dialog?.mode === "add" && (
        <PromptDialog
          title={t("contexts.add")}
          confirmLabel={t("contexts.add")}
          placeholder={t("contexts.addPrompt")}
          onSubmit={async name => { setCtx(await api.contexts.add(name)); close(); }}
          onCancel={close}
        />
      )}
      {dialog?.mode === "addServer" && (
        <PromptDialog
          title={t("contexts.addServer")}
          confirmLabel={t("contexts.addServer")}
          placeholder={t("contexts.addServerPrompt")}
          onSubmit={submitServer}
          onCancel={close}
        />
      )}
      {dialog?.mode === "rename" && (
        <PromptDialog
          title={t("contexts.rename")}
          confirmLabel={t("contexts.rename")}
          initialValue={dialog.c.label}
          placeholder={t("contexts.addPrompt")}
          onSubmit={async name => { setCtx(await api.contexts.rename(dialog.c.id, name)); close(); }}
          onCancel={close}
        />
      )}
      {dialog?.mode === "remove" && (
        <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.45)" }} onClick={close}>
          <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
            <h2 className="text-gray-100 text-base font-semibold mb-2">{t("contexts.remove")}</h2>
            <p className="text-gray-400 text-sm mb-4 break-all">{labelOf(dialog.c)}</p>
            <label className="flex items-center gap-2 text-sm text-gray-300 mb-5">
              <input type="checkbox" checked={deleteFile} onChange={e => setDeleteFile(e.target.checked)} />
              {t("contexts.removeFile")}
            </label>
            <div className="flex justify-end gap-2">
              <button onClick={close} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">{t("dialogs.confirm.cancel")}</button>
              <button onClick={async () => { const c = dialog.c; setCtx(await api.contexts.remove(c.id, deleteFile)); close(); }} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: "#dc2626", color: "white" }}>{t("contexts.remove")}</button>
            </div>
          </div>
        </div>
      )}
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

function McpPage({ settings, onSetSetting }: { settings: AppSettings; onSetSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  // Generate a token on first visit if none exists yet.
  useEffect(() => {
    if (settings.mcpToken === "") onSetSetting("mcpToken", crypto.randomUUID());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const host = settings.mcpBind === "internal" ? "127.0.0.1" : "0.0.0.0";
  const url = `http://${host}:${settings.mcpPort}/mcp`;
  const clientUrl = `http://127.0.0.1:${settings.mcpPort}/mcp`;

  const demo = JSON.stringify(
    {
      mcpServers: {
        notefix: {
          command: "npx",
          args: [
            "-y",
            "mcp-remote",
            clientUrl,
            "--header",
            `Authorization: Bearer ${settings.mcpToken}`,
          ],
        },
      },
    },
    null,
    2,
  );

  const copyDemo = async () => {
    try {
      await navigator.clipboard.writeText(demo);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // clipboard unavailable — ignore
    }
  };

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.mcp.title")}</h1>
      <p className="text-sm text-gray-500 mb-6">{t("settings.mcp.subtitle")}</p>
      <div className="flex flex-col gap-3 max-w-md">
        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("settings.mcp.enabled")}</span>
          <Toggle checked={settings.mcpEnabled ?? false} onChange={() => onSetSetting("mcpEnabled", !settings.mcpEnabled)} label={t("settings.mcp.enabled")} />
        </label>

        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("settings.mcp.reachable")}</span>
          <div className="w-56"><Select value={settings.mcpBind ?? "internal"} options={MCP_BINDS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("mcpBind", v as "internal" | "external")} /></div>
        </label>
        {settings.mcpBind === "external" && (
          <div className="rounded border px-3 py-2 text-xs" style={{ background: "#fee2e2", borderColor: "#fca5a5", color: "#991b1b" }}>
            {t("settings.mcp.externalWarning")}
          </div>
        )}

        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("settings.mcp.port")}</span>
          <input type="number" min={1} max={65535} value={settings.mcpPort ?? 4357} onChange={e => onSetSetting("mcpPort", Math.min(65535, Math.max(1, Number(e.target.value) || 4357)))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "#e7d27a" }} />
        </label>

        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("settings.mcp.authRequired")}</span>
          <Toggle checked={settings.mcpAuthRequired ?? true} onChange={() => onSetSetting("mcpAuthRequired", !settings.mcpAuthRequired)} label={t("settings.mcp.authRequired")} />
        </label>

        <h2 className="text-sm font-semibold text-gray-800 mt-2">{t("settings.mcp.token")}</h2>
        <div className="flex items-center gap-2">
          <input type="text" readOnly value={settings.mcpToken} className="flex-1 bg-white border rounded px-2 py-1 text-xs font-mono" style={{ borderColor: "#e7d27a" }} />
          <button onClick={() => onSetSetting("mcpToken", crypto.randomUUID())} className="shrink-0 px-3 py-1 rounded text-xs font-medium border" style={{ borderColor: "#e7d27a", color: "#1c1917" }}>
            {t("settings.mcp.regenerate")}
          </button>
        </div>

        <label className="flex items-center justify-between gap-4 text-sm text-gray-800 mt-2">
          <span>{t("settings.mcp.allowWrite")}</span>
          <Toggle checked={settings.mcpAllowWrite ?? false} onChange={() => onSetSetting("mcpAllowWrite", !settings.mcpAllowWrite)} label={t("settings.mcp.allowWrite")} />
        </label>

        <h2 className="text-sm font-semibold text-gray-800 mt-2">{t("settings.mcp.status")}</h2>
        <p className="text-xs text-gray-600 break-all font-mono">{url}</p>

        <h2 className="text-sm font-semibold text-gray-800 mt-2">{t("settings.mcp.demo")}</h2>
        <p className="text-xs text-gray-500">{t("settings.mcp.demoHint")}</p>
        <div className="relative">
          <pre className="bg-white border rounded p-3 text-[11px] leading-relaxed font-mono overflow-auto" style={{ borderColor: "#e7d27a" }}>{demo}</pre>
          <button onClick={copyDemo} className="absolute top-2 right-2 px-2 py-0.5 rounded text-[11px] font-medium border" style={{ background: "#fde047", borderColor: "#e7d27a", color: "#1c1917" }}>
            {copied ? t("settings.mcp.copied") : t("settings.mcp.copy")}
          </button>
        </div>
      </div>
    </div>
  );
}
