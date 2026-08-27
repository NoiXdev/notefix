import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import type { IconDefinition } from "@fortawesome/fontawesome-svg-core";
import { faGlobe, faCircleInfo, faPalette, faGear, faPlug, faChartColumn, faKeyboard, faStethoscope, faChevronRight, faDownload, faServer, faLock } from "@fortawesome/free-solid-svg-icons";
import { faAndroid, faApple, faGooglePlay } from "@fortawesome/free-brands-svg-icons";
import { api, type AppInfo, type UpdateInfo } from "../api";
import type { ContextInfo } from "../contexts";
import { startServerAuth } from "../serverAuth";
import type { Stats } from "../types";
import type { DateFormat } from "../dates";
import type { AppSettings } from "../hooks/useSettings";
import { useVault } from "../hooks/useVault";
import Logo from "./Logo";
import Select from "./Select";
import Toggle from "./Toggle";
import ShortcutsSettings from "./ShortcutsSettings";
import PromptDialog from "./PromptDialog";
import VaultSetup from "./VaultSetup";
import VaultUnlock from "./VaultUnlock";
import { runSystemChecks } from "../systemChecks";
import { OSS_LIBS } from "../licenses";
import { useIsMobile } from "../hooks/useIsMobile";
import { isMobilePlatform } from "../platform";

export type Page = "about" | "apps" | "security" | "appearance" | "system" | "contexts" | "mcp" | "stats" | "shortcuts" | "diagnostics";

interface NavItemProps {
  label: string;
  icon: IconDefinition;
  active: boolean;
  mobile?: boolean;
  onClick: () => void;
}

function NavItem({ label, icon, active, mobile, onClick }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={`w-full text-left flex items-center transition-colors ${
        mobile ? "gap-3 px-4 py-3 text-[15px] border-b border-gray-900" : "gap-2.5 px-4 py-2 text-sm"
      } ${active ? "bg-gray-800 text-white" : "text-gray-400 hover:bg-gray-900 hover:text-gray-200"}`}
    >
      {mobile && <FontAwesomeIcon icon={icon} className={`shrink-0 w-5 text-base ${active ? "text-white" : "text-gray-500"}`} />}
      <span className="flex-1 truncate">{label}</span>
      {mobile && <FontAwesomeIcon icon={faChevronRight} className="shrink-0 text-gray-600 text-xs" />}
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

/** "Get Notefix everywhere" — promote the other platforms + the sync server. */
function AppsPage() {
  const { t } = useTranslation();
  const rows: {
    icon: IconDefinition;
    name: string;
    desc: string;
    soon?: boolean;
    action?: { label: string; icon: IconDefinition; url: string };
  }[] = [
    {
      icon: faAndroid,
      name: t("settings.apps.android"),
      desc: t("settings.apps.androidDesc"),
      action: {
        label: t("settings.apps.openPlayStore"),
        icon: faGooglePlay,
        url: "https://play.google.com/store/apps/details?id=dev.noix.notefix",
      },
    },
    { icon: faApple, name: t("settings.apps.ios"), desc: t("settings.apps.iosDesc"), soon: true },
    { icon: faServer, name: t("settings.apps.server"), desc: t("settings.apps.serverDesc"), soon: true },
  ];
  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.apps.title")}</h1>
      <p className="text-sm text-gray-500 mb-6">{t("settings.apps.subtitle")}</p>
      <div className="flex flex-col gap-3 max-w-md">
        {rows.map(r => (
          <div
            key={r.name}
            className="flex items-center gap-4 p-4 rounded-lg border"
            style={{ borderColor: "var(--line-muted)", background: "var(--paper-raised)" }}
          >
            <FontAwesomeIcon icon={r.icon} className="text-2xl w-8 text-center shrink-0" style={{ color: "var(--ink)" }} />
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 flex-wrap">
                <span className="font-semibold text-gray-900">{r.name}</span>
                {r.soon && (
                  <span className="px-2 py-0.5 rounded-full text-[10px] font-medium" style={{ background: "var(--line)", color: "#1c1917" }}>
                    {t("settings.apps.comingSoon")}
                  </span>
                )}
              </div>
              <p className="text-xs text-gray-500 mt-0.5">{r.desc}</p>
            </div>
            {r.action && (
              <button
                onClick={() => api.openExternal(r.action!.url)}
                className="shrink-0 flex items-center gap-1.5 px-3 py-1.5 rounded text-sm font-medium"
                style={{ background: "var(--accent-strong)", color: "#1c1917" }}
              >
                <FontAwesomeIcon icon={r.action.icon} /> {r.action.label}
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

/** In-app "change the vault passphrase" dialog: current + new + confirm-new. */
function ChangePassphraseDialog({ vault, onClose }: { vault: ReturnType<typeof useVault>; onClose: () => void }) {
  const { t } = useTranslation();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    if (!next || next !== confirm) {
      setError(t("vault.mismatch"));
      return;
    }
    setError(null);
    try {
      await vault.changePassphrase(current, next);
      onClose();
    } catch {
      setError(t("security.wrongCurrent"));
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.45)" }} onClick={onClose}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-3">{t("security.changePassphrase")}</h2>
        <input
          type="password"
          autoFocus
          value={current}
          placeholder={t("security.currentPassphrase")}
          onChange={e => setCurrent(e.target.value)}
          className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2"
        />
        <input
          type="password"
          value={next}
          placeholder={t("security.newPassphrase")}
          onChange={e => setNext(e.target.value)}
          className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2"
        />
        <input
          type="password"
          value={confirm}
          placeholder={t("security.confirmNewPassphrase")}
          onChange={e => setConfirm(e.target.value)}
          className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2"
        />
        {error && (
          <div className="text-sm text-red-400 mb-2" role="alert">
            {error}
          </div>
        )}
        <div className="flex justify-end gap-2 mt-2">
          <button onClick={onClose} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
            {t("vault.cancel")}
          </button>
          <button
            onClick={() => void submit()}
            disabled={!current || !next || !confirm}
            className="px-3 py-1.5 rounded text-sm font-medium disabled:opacity-40"
            style={{ background: "var(--line)", color: "#1c1917" }}
          >
            {t("security.save")}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Vault status, actions (setup / lock / change passphrase), biometric toggle + auto-lock config. */
function SecurityPage({ settings, onSetSetting }: {
  settings: AppSettings;
  onSetSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void;
}) {
  const { t } = useTranslation();
  const vault = useVault();
  const [biometricAvailable, setBiometricAvailable] = useState(false);
  const [showSetup, setShowSetup] = useState(false);
  const [showUnlockForBiometric, setShowUnlockForBiometric] = useState(false);
  const [showChangePass, setShowChangePass] = useState(false);

  useEffect(() => {
    api.vault.biometricAvailable().then(setBiometricAvailable);
  }, []);

  const enableBiometric = async () => {
    await api.vault.biometricEnable();
    await vault.refresh();
    onSetSetting("vaultBiometric", true);
  };

  const toggleBiometric = async () => {
    if (vault.status.biometric) {
      await api.vault.biometricDisable();
      await vault.refresh();
      onSetSetting("vaultBiometric", false);
      return;
    }
    if (!vault.status.unlocked) {
      setShowUnlockForBiometric(true);
      return;
    }
    await enableBiometric();
  };

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("security.title")}</h1>
      <p className="text-sm text-gray-500 mb-6">{t("security.subtitle")}</p>

      <div className="flex flex-col gap-3 max-w-md">
        {!vault.status.exists ? (
          <>
            <p className="text-sm text-gray-600">{t("security.notSetUp")}</p>
            <button
              onClick={() => setShowSetup(true)}
              className="self-start px-4 py-1.5 rounded text-sm font-medium"
              style={{ background: "var(--line)", color: "#1c1917" }}
            >
              {t("security.setUp")}
            </button>
          </>
        ) : (
          <>
            <p className="text-sm text-gray-600">{vault.status.unlocked ? t("security.unlocked") : t("security.locked")}</p>
            <div className="flex flex-wrap gap-2">
              <button
                onClick={() => void vault.lock()}
                disabled={!vault.status.unlocked}
                className="px-4 py-1.5 rounded text-sm font-medium border disabled:opacity-40"
                style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}
              >
                {t("security.lockNow")}
              </button>
              <button
                onClick={() => setShowChangePass(true)}
                className="px-4 py-1.5 rounded text-sm font-medium border"
                style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}
              >
                {t("security.changePassphrase")}
              </button>
            </div>

            {biometricAvailable && (
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800 mt-2 max-w-sm">
                <span>{t("security.biometric")}</span>
                <Toggle checked={vault.status.biometric} onChange={() => void toggleBiometric()} label={t("security.biometric")} />
              </label>
            )}
          </>
        )}
      </div>

      <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("security.lockScope")}</h2>
      <div className="max-w-sm">
        <Select
          value={settings.vaultLockScope}
          options={VAULT_LOCK_SCOPES.map(o => ({ value: o.value, label: t(o.labelKey) }))}
          onChange={v => onSetSetting("vaultLockScope", v as import("../hooks/useSettings").VaultLockScope)}
        />
      </div>

      <h2 className="text-sm font-semibold text-gray-800 mt-8 mb-2">{t("security.autoLock")}</h2>
      <div className="flex flex-col gap-3 max-w-sm">
        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("security.autoLockIdle")}</span>
          <Toggle checked={settings.autoLockIdle} onChange={() => onSetSetting("autoLockIdle", !settings.autoLockIdle)} label={t("security.autoLockIdle")} />
        </label>
        {settings.autoLockIdle && (
          <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
            <span>{t("security.autoLockMinutes")}</span>
            <input
              type="number"
              min={1}
              value={settings.autoLockMinutes}
              onChange={e => onSetSetting("autoLockMinutes", Math.max(1, Number(e.target.value) || 1))}
              className="w-24 bg-white border rounded px-2 py-1"
              style={{ borderColor: "var(--line-muted)" }}
            />
          </label>
        )}
        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("security.autoLockOnHide")}</span>
          <Toggle checked={settings.autoLockOnHide} onChange={() => onSetSetting("autoLockOnHide", !settings.autoLockOnHide)} label={t("security.autoLockOnHide")} />
        </label>
        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("security.lockOnSleep")}</span>
          <Toggle checked={settings.autoLockOnSleep} onChange={() => onSetSetting("autoLockOnSleep", !settings.autoLockOnSleep)} label={t("security.lockOnSleep")} />
        </label>
      </div>

      {showSetup && (
        <VaultSetup setup={vault.setup} onSuccess={() => setShowSetup(false)} onCancel={() => setShowSetup(false)} />
      )}
      {showUnlockForBiometric && (
        <VaultUnlock
          biometricAvailable={false}
          unlock={vault.unlock}
          unlockRecovery={vault.unlockRecovery}
          unlockBiometric={vault.unlockBiometric}
          onSuccess={() => { setShowUnlockForBiometric(false); void enableBiometric(); }}
          onCancel={() => setShowUnlockForBiometric(false)}
        />
      )}
      {showChangePass && <ChangePassphraseDialog vault={vault} onClose={() => setShowChangePass(false)} />}
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

// Literal swatch colors per theme (paper + accent) so the picker previews each
// theme regardless of which one is active. Values mirror the :root sets in index.css.
const THEME_OPTIONS: { value: import("../hooks/useSettings").Theme; labelKey: string; paper: string; accent: string }[] = [
  { value: "butter", labelKey: "settings.appearance.themes.butter", paper: "#fef9c3", accent: "#facc15" },
  { value: "orange", labelKey: "settings.appearance.themes.orange", paper: "#ffedd5", accent: "#fb923c" },
  { value: "lavender", labelKey: "settings.appearance.themes.lavender", paper: "#ede9fe", accent: "#a78bfa" },
  { value: "brown", labelKey: "settings.appearance.themes.brown", paper: "#efe6d8", accent: "#c39a6b" },
];

const COUNT_POSITIONS: { value: import("../hooks/useSettings").CountPos; labelKey: string }[] = [
  { value: "topRight", labelKey: "settings.appearance.countPositions.topRight" },
  { value: "topLeft", labelKey: "settings.appearance.countPositions.topLeft" },
  { value: "bottomRight", labelKey: "settings.appearance.countPositions.bottomRight" },
  { value: "bottomLeft", labelKey: "settings.appearance.countPositions.bottomLeft" },
];

const LINE_HEIGHTS: { value: import("../hooks/useSettings").EditorLineHeight; labelKey: string }[] = [
  { value: "normal", labelKey: "settings.appearance.lineHeights.normal" },
  { value: "relaxed", labelKey: "settings.appearance.lineHeights.relaxed" },
  { value: "loose", labelKey: "settings.appearance.lineHeights.loose" },
];

const TOOLBAR_POSITIONS: { value: import("../hooks/useSettings").EditorToolbarPos; labelKey: string }[] = [
  { value: "bottom", labelKey: "settings.appearance.toolbarPositions.bottom" },
  { value: "top", labelKey: "settings.appearance.toolbarPositions.top" },
  { value: "hidden", labelKey: "settings.appearance.toolbarPositions.hidden" },
];

const FONT_SIZES: { value: import("../hooks/useSettings").EditorFontSize; labelKey: string }[] = [
  { value: "small", labelKey: "settings.appearance.fontSizes.small" },
  { value: "medium", labelKey: "settings.appearance.fontSizes.medium" },
  { value: "large", labelKey: "settings.appearance.fontSizes.large" },
  { value: "xlarge", labelKey: "settings.appearance.fontSizes.xlarge" },
];

const FONT_FAMILIES: { value: import("../hooks/useSettings").EditorFontFamily; labelKey: string }[] = [
  { value: "sans", labelKey: "settings.appearance.fontFamilies.sans" },
  { value: "serif", labelKey: "settings.appearance.fontFamilies.serif" },
  { value: "mono", labelKey: "settings.appearance.fontFamilies.mono" },
  { value: "rounded", labelKey: "settings.appearance.fontFamilies.rounded" },
];

const EDITOR_WIDTHS: { value: import("../hooks/useSettings").EditorWidth; labelKey: string }[] = [
  { value: "full", labelKey: "settings.appearance.editorWidths.full" },
  { value: "medium", labelKey: "settings.appearance.editorWidths.medium" },
  { value: "narrow", labelKey: "settings.appearance.editorWidths.narrow" },
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

const MCP_PROTECTED_ACCESS: { value: import("../hooks/useSettings").McpProtectedAccess; labelKey: string }[] = [
  { value: "off", labelKey: "settings.mcp.protectedAccessOptions.off" },
  { value: "read", labelKey: "settings.mcp.protectedAccessOptions.read" },
  { value: "readwrite", labelKey: "settings.mcp.protectedAccessOptions.readwrite" },
];

const VAULT_LOCK_SCOPES: { value: import("../hooks/useSettings").VaultLockScope; labelKey: string }[] = [
  { value: "session", labelKey: "security.lockScopes.session" },
  { value: "perNote", labelKey: "security.lockScopes.perNote" },
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
  // Mobile: drill-down. `navOpen` shows the full-width nav list; picking an
  // entry opens the page full-width with a back button (like list↔editor).
  const isMobile = useIsMobile();
  const [navOpen, setNavOpen] = useState(initialPage == null);
  const openPage = (p: Page) => { setPage(p); setNavOpen(false); };
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
      {(!isMobile || navOpen) && (
      <aside
        className={`${isMobile ? "w-full" : "w-52 shrink-0"} bg-gray-950 flex flex-col h-full select-none`}
        style={{ paddingTop: "env(safe-area-inset-top)", paddingBottom: "env(safe-area-inset-bottom)" }}
      >
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
          <NavItem icon={faCircleInfo} mobile={isMobile} label={t("settings.nav.about")} active={page === "about"} onClick={() => openPage("about")} />
          <NavItem icon={faDownload} mobile={isMobile} label={t("settings.nav.apps")} active={page === "apps"} onClick={() => openPage("apps")} />
          <NavItem icon={faLock} mobile={isMobile} label={t("settings.nav.security")} active={page === "security"} onClick={() => openPage("security")} />
          <NavItem icon={faPalette} mobile={isMobile} label={t("settings.nav.appearance")} active={page === "appearance"} onClick={() => openPage("appearance")} />
          <NavItem icon={faGear} mobile={isMobile} label={t("settings.nav.system")} active={page === "system"} onClick={() => openPage("system")} />
          <NavItem icon={faGlobe} mobile={isMobile} label={t("contexts.nav")} active={page === "contexts"} onClick={() => openPage("contexts")} />
          {!isMobilePlatform && <NavItem icon={faPlug} mobile={isMobile} label={t("settings.nav.mcp")} active={page === "mcp"} onClick={() => openPage("mcp")} />}
          <NavItem icon={faChartColumn} mobile={isMobile} label={t("settings.nav.stats")} active={page === "stats"} onClick={() => openPage("stats")} />
          <NavItem icon={faKeyboard} mobile={isMobile} label={t("settings.nav.shortcuts")} active={page === "shortcuts"} onClick={() => openPage("shortcuts")} />
          <NavItem icon={faStethoscope} mobile={isMobile} label={t("settings.nav.diagnostics")} active={page === "diagnostics"} onClick={() => openPage("diagnostics")} />
        </nav>
      </aside>
      )}

      {(!isMobile || !navOpen) && (
      <main className="flex-1 flex flex-col min-w-0 overflow-hidden" style={{ background: "var(--paper)", paddingBottom: "env(safe-area-inset-bottom)" }}>
        {isMobile && (
          <button
            onClick={() => setNavOpen(true)}
            className="shrink-0 flex items-center gap-1.5 px-4 pb-3 text-[15px] font-medium border-b"
            style={{ background: "var(--panel)", borderColor: "var(--line)", color: "var(--ink)", paddingTop: "calc(0.75rem + env(safe-area-inset-top))" }}
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="15 18 9 12 15 6" /></svg>
            {t("settings.sidebarTitle")}
          </button>
        )}
        <div className="settings-scroll flex-1 overflow-auto px-4 py-6 sm:px-10 sm:py-10">
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

            {!isMobilePlatform && <UpdateChecker settings={settings} onSetSetting={onSetSetting} />}

            <div className="mt-10 max-w-md">
              <h2 className="text-sm font-semibold text-gray-800 mb-1">{t("settings.about.openSource")}</h2>
              <p className="text-xs text-gray-500 mb-3">{t("settings.about.openSourceIntro")}</p>
              <ul className="flex flex-col divide-y divide-yellow-200/70 border-y border-[var(--line-muted)]/70">
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

        {page === "apps" && <AppsPage />}

        {page === "security" && <SecurityPage settings={settings} onSetSetting={onSetSetting} />}

        {page === "appearance" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.appearance.title")}</h1>
            <p className="text-sm text-gray-500 mb-6">{t("settings.appearance.subtitle")}</p>

            <h2 className="text-sm font-semibold text-gray-800 mb-2">{t("settings.appearance.theme")}</h2>
            <div className="flex flex-wrap gap-2 mb-8">
              {THEME_OPTIONS.map(o => (
                <button
                  key={o.value}
                  onClick={() => onSetSetting("theme", o.value)}
                  className={`flex flex-col items-center gap-1.5 rounded-lg border p-2 transition-colors ${settings.theme === o.value ? "border-gray-800 ring-2 ring-gray-800" : "border-gray-200 hover:border-gray-400"}`}
                  title={t(o.labelKey)}
                >
                  <span className="flex h-9 w-14 overflow-hidden rounded border border-gray-300">
                    <span className="flex-1" style={{ background: o.paper }} />
                    <span style={{ width: 12, background: o.accent }} />
                  </span>
                  <span className="text-xs text-gray-700">{t(o.labelKey)}</span>
                </button>
              ))}
            </div>

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

            <h1 className="text-lg font-bold text-gray-900 mt-10 mb-4">{t("settings.appearance.editorTitle")}</h1>

            <h2 className="text-sm font-semibold text-gray-800 mb-2">{t("settings.appearance.toolbar")}</h2>
            <div className="max-w-sm mb-6"><Select value={settings.editorToolbarPos} options={TOOLBAR_POSITIONS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorToolbarPos", v as import("../hooks/useSettings").EditorToolbarPos)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mb-2">{t("settings.appearance.fontSize")}</h2>
            <div className="max-w-sm mb-6"><Select value={settings.editorFontSize} options={FONT_SIZES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorFontSize", v as import("../hooks/useSettings").EditorFontSize)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mb-2">{t("settings.appearance.fontFamily")}</h2>
            <div className="max-w-sm mb-6"><Select value={settings.editorFontFamily} options={FONT_FAMILIES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorFontFamily", v as import("../hooks/useSettings").EditorFontFamily)} /></div>

            <h2 className="text-sm font-semibold text-gray-800 mb-2">{t("settings.appearance.editorWidth")}</h2>
            <div className="max-w-sm mb-6"><Select value={settings.editorWidth} options={EDITOR_WIDTHS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorWidth", v as import("../hooks/useSettings").EditorWidth)} /></div>

            <div className="flex flex-col gap-3 max-w-sm mb-4">
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.appearance.charCount")}</span>
                <Toggle checked={settings.editorCountShow} onChange={() => onSetSetting("editorCountShow", !settings.editorCountShow)} label={t("settings.appearance.charCount")} />
              </label>
              {settings.editorCountShow && (
                <Select value={settings.editorCountPos} options={COUNT_POSITIONS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorCountPos", v as import("../hooks/useSettings").CountPos)} />
              )}
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.appearance.invisibles")}</span>
                <Toggle checked={settings.editorInvisibles} onChange={() => onSetSetting("editorInvisibles", !settings.editorInvisibles)} label={t("settings.appearance.invisibles")} />
              </label>
            </div>

            <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-2">{t("settings.appearance.lineHeight")}</h2>
            <div className="max-w-sm"><Select value={settings.editorLineHeight} options={LINE_HEIGHTS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorLineHeight", v as import("../hooks/useSettings").EditorLineHeight)} /></div>
          </div>
        )}

        {page === "system" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.system.title")}</h1>
            <p className="text-sm text-gray-500 mb-6">{t("settings.system.subtitle")}</p>
            <div className="flex flex-col gap-3 max-w-md">
              {!isMobilePlatform && (
              <>
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
              </>
              )}
              <button
                onClick={() => onExport([], "notefix-export.json")}
                className="mt-2 self-start px-4 py-1.5 rounded text-sm font-medium"
                style={{ background: "var(--line)", color: "#1c1917" }}
              >
                {t("settings.system.exportAll")}
              </button>

              {!isMobilePlatform && (
              <>
              <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-1">{t("settings.system.location")}</h2>
              <p className="text-xs text-gray-600 break-all mb-2">{dbPath}</p>
              <button
                onClick={changeLocation}
                className="self-start px-4 py-1.5 rounded text-sm font-medium border"
                style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}
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
                    style={{ background: "var(--line)", color: "#1c1917" }}
                  >
                    {t("settings.system.restartNow")}
                  </button>
                </div>
              )}
              </>
              )}
              <h2 className="text-sm font-semibold text-gray-800 mt-6 mb-1">{t("settings.system.editorAndHistory")}</h2>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.autosaveDelay")}</span>
                <input type="number" min={100} step={50} value={settings.autosaveDelay ?? 400} onChange={e => onSetSetting("autosaveDelay", Math.max(100, Number(e.target.value) || 400))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "var(--line-muted)" }} />
              </label>
              <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
                <span>{t("settings.system.revisionLimit")}</span>
                <input type="number" min={1} value={settings.revisionLimit ?? 50} onChange={e => onSetSetting("revisionLimit", Math.max(1, Number(e.target.value) || 50))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "var(--line-muted)" }} />
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
                <input type="number" min={1} value={settings.trashRetentionDays ?? 30} onChange={e => onSetSetting("trashRetentionDays", Math.max(1, Number(e.target.value) || 30))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "var(--line-muted)" }} />
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
        </div>
      </main>
      )}
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
          <div key={c.id} className="flex items-start justify-between gap-3 rounded border px-3 py-2" style={{ borderColor: "var(--line-muted)", background: "var(--paper-raised)" }}>
            <div className="min-w-0">
              <div className="flex items-center gap-2 text-sm font-medium text-gray-800">
                {c.kind === "server" && <FontAwesomeIcon icon={faGlobe} className="text-[11px] text-gray-500 shrink-0" />}
                <span className="truncate">{labelOf(c)}</span>
                {c.active && (
                  <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide" style={{ background: "var(--line)", color: "#1c1917" }}>{t("contexts.active")}</span>
                )}
              </div>
              <div className="text-xs text-gray-500 break-all font-mono">{c.kind === "server" ? c.serverUrl : c.path}</div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <button onClick={() => setDialog({ mode: "rename", c })} className="px-3 py-1 rounded text-xs font-medium border" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>{t("contexts.rename")}</button>
              <button onClick={() => setDialog({ mode: "remove", c })} disabled={c.active} className="px-3 py-1 rounded text-xs font-medium border disabled:opacity-40 disabled:cursor-not-allowed" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>{t("contexts.remove")}</button>
            </div>
          </div>
        ))}
      </div>
      <div className="mt-4 flex items-center gap-2">
        <button onClick={() => setDialog({ mode: "add" })} className="px-4 py-1.5 rounded text-sm font-medium" style={{ background: "var(--line)", color: "#1c1917" }}>{t("contexts.add")}</button>
        <button onClick={() => { setError(null); setDialog({ mode: "addServer" }); }} className="px-4 py-1.5 rounded text-sm font-medium border" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>{t("contexts.addServer")}</button>
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
              <button onClick={onChangeLocation} className="shrink-0 px-3 py-1 rounded text-xs font-medium border" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>{t("diagnostics.changeLocation")}</button>
            )}
          </div>
        ))}
        <button onClick={run} className="self-start mt-2 px-4 py-1.5 rounded text-sm font-medium border" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>{t("diagnostics.recheck")}</button>
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
          <input type="number" min={1} max={65535} value={settings.mcpPort ?? 4357} onChange={e => onSetSetting("mcpPort", Math.min(65535, Math.max(1, Number(e.target.value) || 4357)))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "var(--line-muted)" }} />
        </label>

        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("settings.mcp.authRequired")}</span>
          <Toggle checked={settings.mcpAuthRequired ?? true} onChange={() => onSetSetting("mcpAuthRequired", !settings.mcpAuthRequired)} label={t("settings.mcp.authRequired")} />
        </label>

        <h2 className="text-sm font-semibold text-gray-800 mt-2">{t("settings.mcp.token")}</h2>
        <div className="flex items-center gap-2">
          <input type="text" readOnly value={settings.mcpToken} className="flex-1 bg-white border rounded px-2 py-1 text-xs font-mono" style={{ borderColor: "var(--line-muted)" }} />
          <button onClick={() => onSetSetting("mcpToken", crypto.randomUUID())} className="shrink-0 px-3 py-1 rounded text-xs font-medium border" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>
            {t("settings.mcp.regenerate")}
          </button>
        </div>

        <label className="flex items-center justify-between gap-4 text-sm text-gray-800 mt-2">
          <span>{t("settings.mcp.allowWrite")}</span>
          <Toggle checked={settings.mcpAllowWrite ?? false} onChange={() => onSetSetting("mcpAllowWrite", !settings.mcpAllowWrite)} label={t("settings.mcp.allowWrite")} />
        </label>

        <label className="flex items-center justify-between gap-4 text-sm text-gray-800">
          <span>{t("settings.mcp.protectedAccess")}</span>
          <div className="w-40">
            <Select
              value={settings.mcpProtectedAccess ?? "off"}
              options={MCP_PROTECTED_ACCESS.map(o => ({ value: o.value, label: t(o.labelKey) }))}
              onChange={v => onSetSetting("mcpProtectedAccess", v as import("../hooks/useSettings").McpProtectedAccess)}
            />
          </div>
        </label>
        {settings.mcpProtectedAccess !== "off" && (
          <div className="rounded border px-3 py-2 text-xs" style={{ background: "#fee2e2", borderColor: "#fca5a5", color: "#991b1b" }}>
            {t("settings.mcp.protectedAccessWarning")}
          </div>
        )}

        <h2 className="text-sm font-semibold text-gray-800 mt-2">{t("settings.mcp.status")}</h2>
        <p className="text-xs text-gray-600 break-all font-mono">{url}</p>

        <h2 className="text-sm font-semibold text-gray-800 mt-2">{t("settings.mcp.demo")}</h2>
        <p className="text-xs text-gray-500">{t("settings.mcp.demoHint")}</p>
        <div className="relative">
          <pre className="bg-white border rounded p-3 text-[11px] leading-relaxed font-mono overflow-auto" style={{ borderColor: "var(--line-muted)" }}>{demo}</pre>
          <button onClick={copyDemo} className="absolute top-2 right-2 px-2 py-0.5 rounded text-[11px] font-medium border" style={{ background: "var(--line)", borderColor: "var(--line-muted)", color: "#1c1917" }}>
            {copied ? t("settings.mcp.copied") : t("settings.mcp.copy")}
          </button>
        </div>
      </div>
    </div>
  );
}
