import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import type { IconDefinition } from "@fortawesome/fontawesome-svg-core";
import { faGlobe, faCircleInfo, faPalette, faGear, faPlug, faChartColumn, faKeyboard, faStethoscope, faChevronRight, faDownload, faServer, faLock } from "@fortawesome/free-solid-svg-icons";
import { faAndroid, faApple, faGooglePlay } from "@fortawesome/free-brands-svg-icons";
import { api, type AppInfo, type UpdateInfo, type ReleaseInfo } from "../api";
import type { ContextInfo } from "../contexts";
import { startServerAuth } from "../serverAuth";
import type { Stats, RotationCode, InviteCode, RecoveryCreated } from "../types";
import type { DateFormat } from "../dates";
import type { AppSettings } from "../hooks/useSettings";
import { useVault } from "../hooks/useVault";
import Logo from "./Logo";
import Select from "./Select";
import Toggle from "./Toggle";
import ShortcutsSettings from "./ShortcutsSettings";
import PromptDialog from "./PromptDialog";
import ContextMenu from "./ContextMenu";
import VaultSetup from "./VaultSetup";
import VaultUnlock from "./VaultUnlock";
import VaultInviteCodeDialog from "./VaultInviteCodeDialog";
import VaultAcceptInviteDialog from "./VaultAcceptInviteDialog";
import VaultRotateDialog from "./VaultRotateDialog";
import VaultCodesDialog from "./VaultCodesDialog";
import VaultRotationRedeemDialog from "./VaultRotationRedeemDialog";
import VaultConflictDialog from "./VaultConflictDialog";
import VaultRecoveryKeyDialog from "./VaultRecoveryKeyDialog";
import WhatsNew from "./WhatsNew";
import { runSystemChecks } from "../systemChecks";
import { OSS_LIBS } from "../licenses";
import { useIsMobile } from "../hooks/useIsMobile";
import { isMobilePlatform } from "../platform";
import SettingsTabs from "./settings/SettingsTabs";
import SettingsSection from "./settings/SettingsSection";
import SettingRow from "./settings/SettingRow";
import SettingsGrid from "./settings/SettingsGrid";

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
    <>
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
      <SettingRow label={t("update.onStart")}>
        <Toggle
          checked={settings.checkUpdatesOnStart}
          onChange={() => onSetSetting("checkUpdatesOnStart", !settings.checkUpdatesOnStart)}
          label={t("update.onStart")}
        />
      </SettingRow>
    </>
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
      <SettingsSection title={t("settings.apps.sections.platforms")}>
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
      </SettingsSection>
    </div>
  );
}

/**
 * In-app "change a vault passphrase" dialog: current + new + confirm-new.
 * Generalized over `onSubmit` so it can rewrap either the active context's
 * vault (Security page, via `vault.changePassphrase`) or a specific,
 * possibly non-active, context's vault (Contexts page, via
 * `api.contexts.vaultChangePassphrase`).
 */
function ChangePassphraseDialog({ onSubmit, onClose, title }: {
  onSubmit: (current: string, next: string) => Promise<void>;
  onClose: () => void;
  title?: string;
}) {
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
      await onSubmit(current, next);
      onClose();
    } catch (e) {
      // A context switch mid-request aborts before a single local write, so
      // the passphrase is unchanged on both sides — that is a retry, not a
      // wrong current passphrase.
      const msg = e instanceof Error ? e.message : String(e ?? "");
      setError(
        msg.includes("context changed during the request")
          ? t("common.contextChanged")
          : t("security.wrongCurrent"),
      );
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.45)" }} onClick={onClose}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-3">{title ?? t("security.changePassphrase")}</h2>
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
  const [activeContextLabel, setActiveContextLabel] = useState<string | null>(null);
  const [showRecoveryFollowup, setShowRecoveryFollowup] = useState(false);
  const [recoveryNotice, setRecoveryNotice] = useState<string | null>(null);
  const [showRotationRedeem, setShowRotationRedeem] = useState(false);
  const [showConflict, setShowConflict] = useState(false);
  const [recoveryCreated, setRecoveryCreated] = useState<RecoveryCreated | null>(null);
  const [recoveryBusy, setRecoveryBusy] = useState(false);

  useEffect(() => {
    api.vault.biometricAvailable().then(setBiometricAvailable);
  }, []);

  const createRecovery = async () => {
    if (recoveryBusy) return;
    setRecoveryBusy(true);
    try {
      setRecoveryNotice(null);
      setRecoveryCreated(await vault.recoveryCreate());
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e ?? "");
      setRecoveryNotice(t("vault.recovery.failed", { error: msg }));
    } finally {
      setRecoveryBusy(false);
    }
  };

  // The vault this page manages is always the ACTIVE context's — name it
  // explicitly, since every other context's vault now lives under Contexts.
  useEffect(() => {
    void api.contexts.list().then(list => {
      const active = list.find(c => c.active);
      if (!active) return;
      setActiveContextLabel(active.label || (active.kind === "server" ? active.serverUrl : t("contexts.localDefault")));
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
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

  const [tab, setTab] = useState<"vault" | "autoLock">("vault");
  const tabs = [
    { id: "vault", label: t("security.tabs.vault") },
    { id: "autoLock", label: t("security.tabs.autoLock") },
  ];

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("security.title")}</h1>
      <p className="text-sm text-gray-500 mb-1">{t("security.subtitle")}</p>
      {activeContextLabel && (
        <p className="text-sm text-gray-600">{t("security.forContext", { name: activeContextLabel })}</p>
      )}
      <p className="text-xs text-gray-500 mb-6">{t("security.otherContextsHint")}</p>

      {vault.status.conflict && (
        <div
          role="status"
          className="mb-6 rounded border px-3 py-2 text-sm"
          style={{ borderColor: "#d97706", background: "#fffbeb", color: "#7c2d12" }}
        >
          <span>{t("vault.conflict.hint")}</span>
          <button
            onClick={() => setShowConflict(true)}
            className="ml-3 px-3 py-1 rounded text-xs font-medium border"
            style={{ borderColor: "#d97706", color: "#7c2d12" }}
          >
            {t("vault.conflict.resolve")}
          </button>
        </div>
      )}

      {/* The one surface that survives every way in: a member who unlocked
          with Touch ID (which types no passphrase) or postponed the step
          during the unlock still finds their rotation code here.

          Redeeming re-wraps the new key under the passphrase and needs the
          vault open, so a locked vault gets the hint instead of the button —
          the backend would refuse with "vault locked", and a one-time code
          the user hand-carried must never be reported as burnt. */}
      {vault.status.rotationCode && (
        <div
          role="status"
          className="mb-6 rounded border px-3 py-2 text-sm flex flex-wrap items-center gap-3"
          style={{ borderColor: "#d97706", background: "#fffbeb", color: "#7c2d12" }}
        >
          <span>{t("vault.rotation.codeBanner")}</span>
          {vault.status.unlocked ? (
            <button
              onClick={() => setShowRotationRedeem(true)}
              className="px-3 py-1 rounded text-xs font-medium border"
              style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}
            >
              {t("vault.rotation.enterCode")}
            </button>
          ) : (
            <span className="text-xs">{t("vault.rotation.lockedHint")}</span>
          )}
        </div>
      )}

      {/* Somebody else rotated the key: only the holder of the recovery key
          can carry it over to the new generation, so only they see this. */}
      {vault.status.recoveryMissing && (
        <div
          role="status"
          className="mb-6 rounded border px-3 py-2 text-sm flex flex-wrap items-center gap-3"
          style={{ borderColor: "#d97706", background: "#fffbeb", color: "#7c2d12" }}
        >
          <span>{t("vault.recovery.missing")}</span>
          {/* The follow-up wraps DEKs taken from the live ring, so a locked
              vault would be refused by the backend with "vault locked" —
              offer the hint rather than a button that cannot work. */}
          {vault.status.unlocked ? (
            <button
              onClick={() => { setRecoveryNotice(null); setShowRecoveryFollowup(true); }}
              className="px-3 py-1 rounded text-xs font-medium border"
              style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}
            >
              {t("vault.recovery.submit")}
            </button>
          ) : (
            <span className="text-xs">{t("vault.lockedHint")}</span>
          )}
        </div>
      )}

      {/* This owner holds no recovery key of their own yet — offer to mint
          one, wrapping every ring generation the vault currently holds. */}
      {vault.status.recoveryEligible && (
        <div role="status" className="mb-6 rounded border px-3 py-2 text-sm flex flex-wrap items-center gap-3" style={{ borderColor: "var(--line-muted)", background: "var(--paper-raised)" }}>
          <span>{t("vault.recovery.none")}</span>
          <button onClick={() => void createRecovery()} disabled={recoveryBusy} className="px-3 py-1 rounded text-xs font-medium border disabled:opacity-40" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>
            {t("vault.recovery.create")}
          </button>
        </div>
      )}
      {recoveryNotice && (
        <div role="status" className="mb-6 text-sm text-gray-600">{recoveryNotice}</div>
      )}

      <SettingsTabs tabs={tabs} active={tab} onChange={id => setTab(id as "vault" | "autoLock")} />

      {tab === "vault" && (
        <SettingsGrid>
          <SettingsSection title={t("security.sections.status")}>
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
                <p className="mt-2 text-xs text-gray-500">{t("security.passphraseHint")}</p>
              </>
            )}
          </SettingsSection>

          <SettingsSection title={t("security.sections.options")}>
            <SettingRow label={t("security.lockScope")}>
              <Select
                value={settings.vaultLockScope}
                options={VAULT_LOCK_SCOPES.map(o => ({ value: o.value, label: t(o.labelKey) }))}
                onChange={v => onSetSetting("vaultLockScope", v as import("../hooks/useSettings").VaultLockScope)}
              />
            </SettingRow>
            {biometricAvailable && vault.status.exists && (
              <SettingRow label={t("security.biometric")}>
                <Toggle checked={vault.status.biometric} onChange={() => void toggleBiometric()} label={t("security.biometric")} />
              </SettingRow>
            )}
          </SettingsSection>
        </SettingsGrid>
      )}

      {tab === "autoLock" && (
        <SettingsGrid>
          <SettingsSection title={t("security.autoLock")}>
            <SettingRow label={t("security.autoLockIdle")}>
              <Toggle checked={settings.autoLockIdle} onChange={() => onSetSetting("autoLockIdle", !settings.autoLockIdle)} label={t("security.autoLockIdle")} />
            </SettingRow>
            {settings.autoLockIdle && (
              <SettingRow label={t("security.autoLockMinutes")}>
                <input
                  type="number"
                  min={1}
                  value={settings.autoLockMinutes}
                  onChange={e => onSetSetting("autoLockMinutes", Math.max(1, Number(e.target.value) || 1))}
                  className="w-24 bg-white border rounded px-2 py-1"
                  style={{ borderColor: "var(--line-muted)" }}
                />
              </SettingRow>
            )}
            <SettingRow label={t("security.autoLockOnHide")}>
              <Toggle checked={settings.autoLockOnHide} onChange={() => onSetSetting("autoLockOnHide", !settings.autoLockOnHide)} label={t("security.autoLockOnHide")} />
            </SettingRow>
            <SettingRow label={t("security.lockOnSleep")}>
              <Toggle checked={settings.autoLockOnSleep} onChange={() => onSetSetting("autoLockOnSleep", !settings.autoLockOnSleep)} label={t("security.lockOnSleep")} />
            </SettingRow>
          </SettingsSection>
        </SettingsGrid>
      )}

      {showSetup && (
        <VaultSetup setup={vault.setup} onSuccess={() => setShowSetup(false)} onCancel={() => setShowSetup(false)} />
      )}
      {showUnlockForBiometric && (
        <VaultUnlock
          biometricAvailable={false}
          recoveryAvailable={vault.status.recoveryHolder}
          unlock={vault.unlock}
          unlockRecovery={vault.unlockRecovery}
          unlockBiometric={vault.unlockBiometric}
          onSuccess={() => { setShowUnlockForBiometric(false); void enableBiometric(); }}
          onCancel={() => setShowUnlockForBiometric(false)}
        />
      )}
      {showChangePass && (
        <ChangePassphraseDialog
          onSubmit={(current, next) => vault.changePassphrase(current, next)}
          onClose={() => setShowChangePass(false)}
        />
      )}
      {showRotationRedeem && (
        <VaultRotationRedeemDialog
          redeem={vault.redeemRotation}
          onSuccess={() => setShowRotationRedeem(false)}
          onCancel={() => setShowRotationRedeem(false)}
        />
      )}
      {showConflict && (
        <VaultConflictDialog resolve={vault.resolveConflict} onClose={() => setShowConflict(false)} />
      )}
      {showRecoveryFollowup && (
        <PromptDialog
          title={t("vault.recovery.submit")}
          confirmLabel={t("vault.recovery.submit")}
          placeholder={t("vault.recoveryKey")}
          onSubmit={async key => {
            setShowRecoveryFollowup(false);
            try {
              await vault.recoveryFollowup(key);
              setRecoveryNotice(t("vault.recovery.added"));
            } catch (e) {
              const msg = e instanceof Error ? e.message : String(e ?? "");
              setRecoveryNotice(
                msg.includes("vault locked")
                  ? t("vault.lockedHint")
                  : msg.includes("wrong recovery key")
                    ? t("vault.wrongRecoveryKey")
                    : t("vault.recovery.failed", { error: msg }),
              );
            }
          }}
          onCancel={() => setShowRecoveryFollowup(false)}
        />
      )}
      {recoveryCreated && (
        <VaultRecoveryKeyDialog
          groups={recoveryCreated.groups}
          incomplete={recoveryCreated.incomplete}
          onClose={() => setRecoveryCreated(null)}
        />
      )}
    </div>
  );
}

/** Theme, language, sidebar/list layout and editor look — grouped into tabs. */
function AppearancePage({ settings, onSetSetting }: {
  settings: AppSettings;
  onSetSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void;
}) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<"general" | "list" | "editor">("general");
  const tabs = [
    { id: "general", label: t("settings.appearance.tabs.general") },
    { id: "list", label: t("settings.appearance.tabs.list") },
    { id: "editor", label: t("settings.appearance.tabs.editor") },
  ];

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.appearance.title")}</h1>
      <p className="text-sm text-gray-500 mb-6">{t("settings.appearance.subtitle")}</p>

      <SettingsTabs tabs={tabs} active={tab} onChange={id => setTab(id as "general" | "list" | "editor")} />

      {tab === "general" && (
        <SettingsGrid>
          <SettingsSection title={t("settings.appearance.sections.look")}>
            <SettingRow label={t("settings.appearance.theme")} stack>
              <div className="flex flex-wrap gap-2">
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
            </SettingRow>
            <SettingRow label={t("settings.appearance.language")}>
              <Select value={settings.language} options={LANGUAGES.map(l => ({ value: l.value, label: l.value === "system" ? t("settings.appearance.langAuto") : l.label }))} onChange={v => onSetSetting("language", v as import("../hooks/useSettings").LangSetting)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.dateFormat")}>
              <Select value={settings.dateFormat} options={DATE_FORMATS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("dateFormat", v as DateFormat)} />
            </SettingRow>
          </SettingsSection>

          <SettingsSection title={t("settings.appearance.sections.content")}>
            <SettingRow label={t("settings.appearance.copyFormat")}>
              <Select value={settings.copyFormat ?? "md"} options={COPY_FORMATS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("copyFormat", v as import("../copyFormat").CopyFormat)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.linkPreview")}>
              <Toggle checked={settings.linkPreviewEnabled ?? true} onChange={() => onSetSetting("linkPreviewEnabled", !settings.linkPreviewEnabled)} label={t("settings.appearance.linkPreview")} />
            </SettingRow>
            <Select value={settings.linkPreviewMode ?? "card"} options={LINK_PREVIEW_MODES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("linkPreviewMode", v as "url" | "inline" | "card")} />
          </SettingsSection>
        </SettingsGrid>
      )}

      {tab === "list" && (
        <SettingsGrid>
          <SettingsSection title={t("settings.appearance.sections.sidebar")}>
            <SettingRow label={t("settings.appearance.sidebarMode")}>
              <Select value={settings.sidebarMode} options={SIDEBAR_MODES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("sidebarMode", v as import("../hooks/useSettings").SidebarMode)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.sidebarSide")}>
              <Select value={settings.sidebarSide} options={SIDEBAR_SIDES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("sidebarSide", v as import("../hooks/useSettings").SidebarSide)} />
            </SettingRow>
          </SettingsSection>

          <SettingsSection title={t("settings.appearance.sections.noteList")}>
            <SettingRow label={t("settings.appearance.pinned")}>
              <Select value={settings.pinnedScope} options={PIN_SCOPES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("pinnedScope", v as import("../hooks/useSettings").PinnedScope)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.folderColor")}>
              <Select value={settings.folderColorStyle} options={FOLDER_COLOR_STYLES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("folderColorStyle", v as import("../hooks/useSettings").FolderColorStyle)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.compactTree")}>
              <Toggle checked={settings.compactTree ?? false} onChange={() => onSetSetting("compactTree", !settings.compactTree)} label={t("settings.appearance.compactTree")} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.treeProgress")}>
              <Toggle checked={settings.treeProgress ?? true} onChange={() => onSetSetting("treeProgress", !settings.treeProgress)} label={t("settings.appearance.treeProgress")} />
            </SettingRow>
          </SettingsSection>
        </SettingsGrid>
      )}

      {tab === "editor" && (
        <SettingsGrid>
          <SettingsSection title={t("settings.appearance.sections.text")}>
            <SettingRow label={t("settings.appearance.fontSize")}>
              <Select value={settings.editorFontSize} options={FONT_SIZES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorFontSize", v as import("../hooks/useSettings").EditorFontSize)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.fontFamily")}>
              <Select value={settings.editorFontFamily} options={FONT_FAMILIES.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorFontFamily", v as import("../hooks/useSettings").EditorFontFamily)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.editorWidth")}>
              <Select value={settings.editorWidth} options={EDITOR_WIDTHS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorWidth", v as import("../hooks/useSettings").EditorWidth)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.lineHeight")}>
              <Select value={settings.editorLineHeight} options={LINE_HEIGHTS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorLineHeight", v as import("../hooks/useSettings").EditorLineHeight)} />
            </SettingRow>
          </SettingsSection>

          <SettingsSection title={t("settings.appearance.sections.tools")}>
            <SettingRow label={t("settings.appearance.toolbar")}>
              <Select value={settings.editorToolbarPos} options={TOOLBAR_POSITIONS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorToolbarPos", v as import("../hooks/useSettings").EditorToolbarPos)} />
            </SettingRow>
            <SettingRow label={t("settings.appearance.charCount")}>
              <Toggle checked={settings.editorCountShow} onChange={() => onSetSetting("editorCountShow", !settings.editorCountShow)} label={t("settings.appearance.charCount")} />
            </SettingRow>
            {settings.editorCountShow && (
              <Select value={settings.editorCountPos} options={COUNT_POSITIONS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("editorCountPos", v as import("../hooks/useSettings").CountPos)} />
            )}
            <SettingRow label={t("settings.appearance.invisibles")}>
              <Toggle checked={settings.editorInvisibles} onChange={() => onSetSetting("editorInvisibles", !settings.editorInvisibles)} label={t("settings.appearance.invisibles")} />
            </SettingRow>
          </SettingsSection>
        </SettingsGrid>
      )}
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

const SIDEBAR_SIDES: { value: import("../hooks/useSettings").SidebarSide; labelKey: string }[] = [
  { value: "left", labelKey: "settings.appearance.sidebarLeft" },
  { value: "right", labelKey: "settings.appearance.sidebarRight" },
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

  // Manual "What's New" link on the About page: fetches on demand and shows
  // the newest releases (not gated to "since a version" like the startup
  // popup in App.tsx — this is just "show me the changelog").
  const [whatsNewState, setWhatsNewState] = useState<"idle" | "loading" | "error" | ReleaseInfo[]>("idle");
  const openWhatsNew = () => {
    setWhatsNewState("loading");
    api.githubReleases().then(rs => setWhatsNewState(rs.slice(0, 10))).catch(() => setWhatsNewState("error"));
  };

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
            <p className="text-sm text-gray-500 mb-6">
              {t("settings.about.version", { version: info.version })}
              {" · "}
              <button
                onClick={openWhatsNew}
                disabled={whatsNewState === "loading"}
                className="text-blue-700 underline disabled:opacity-50"
              >
                {t("settings.about.whatsNew")}
              </button>
            </p>
            {whatsNewState === "error" && <p className="text-xs text-red-600 mb-2">{t("whatsNew.error")}</p>}

            {Array.isArray(whatsNewState) && (
              <WhatsNew releases={whatsNewState} onClose={() => setWhatsNewState("idle")} />
            )}

            <SettingsGrid>
              <SettingsSection title={t("settings.about.sections.info")}>
                <p className="text-sm text-gray-600">{info.description}</p>
                <p className="text-sm text-gray-600">{t("settings.about.story")}</p>
                <div className="flex flex-col gap-1 text-sm">
                  <a href="https://noix.dev" className="text-blue-700 underline">{t("settings.about.project")}</a>
                  <a href="https://docs.noix.dev" className="text-blue-700 underline">{t("settings.about.docs")}</a>
                  <span className="text-gray-500 mt-1">{t("settings.about.license")}</span>
                </div>
              </SettingsSection>

              <div className="md:col-span-2">
                <SettingsSection title={t("settings.about.openSource")}>
                  <p className="text-xs text-gray-500 -mt-1">{t("settings.about.openSourceIntro")}</p>
                  <ul className="flex flex-col divide-y divide-yellow-200/70 border-y border-[var(--line-muted)]/70">
                    {OSS_LIBS.map(lib => (
                      <li key={lib.name} className="flex items-center justify-between gap-3 py-1.5 text-xs">
                        <a href={lib.url} className="text-blue-700 underline">{lib.name}</a>
                        <span className="text-gray-500 whitespace-nowrap">{lib.license}</span>
                      </li>
                    ))}
                  </ul>
                </SettingsSection>
              </div>
            </SettingsGrid>
          </div>
        )}

        {page === "apps" && <AppsPage />}

        {page === "security" && <SecurityPage settings={settings} onSetSetting={onSetSetting} />}

        {page === "appearance" && <AppearancePage settings={settings} onSetSetting={onSetSetting} />}

        {page === "system" && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.system.title")}</h1>
            <p className="text-sm text-gray-500 mb-6">{t("settings.system.subtitle")}</p>

            <SettingsGrid>
              {!isMobilePlatform && (
                <SettingsSection title={t("settings.system.sections.start")}>
                  <SettingRow label={t("settings.system.startOnBoot")}>
                    <Toggle checked={bootEnabled} onChange={toggleBoot} label={t("settings.system.startOnBoot")} />
                  </SettingRow>
                  <SettingRow label={t("settings.system.startMinimized")}>
                    <Toggle checked={settings.startMinimized} onChange={() => onSetSetting("startMinimized", !settings.startMinimized)} label={t("settings.system.startMinimized")} />
                  </SettingRow>
                  <SettingRow label={t("settings.system.closeBehavior")}>
                    <div className="w-56"><Select value={settings.closeAction ?? "ask"} options={CLOSE_ACTIONS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("closeAction", v as import("../hooks/useSettings").CloseAction)} /></div>
                  </SettingRow>
                </SettingsSection>
              )}

              <SettingsSection title={t("settings.system.sections.storage")}>
                <button
                  onClick={() => onExport([], "notefix-export.json")}
                  className="self-start px-4 py-1.5 rounded text-sm font-medium"
                  style={{ background: "var(--line)", color: "#1c1917" }}
                >
                  {t("settings.system.exportAll")}
                </button>

                {!isMobilePlatform && (
                  <>
                    <SettingRow label={t("settings.system.location")} hint={dbPath} stack>
                      <button
                        onClick={changeLocation}
                        className="self-start px-4 py-1.5 rounded text-sm font-medium border"
                        style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}
                      >
                        {t("settings.system.change")}
                      </button>
                    </SettingRow>
                    {locResult && (
                      <div className="text-sm text-gray-700">
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

                <SettingRow label={t("settings.system.trashEnabled")}>
                  <Toggle checked={settings.trashEnabled ?? true} onChange={() => onSetSetting("trashEnabled", !settings.trashEnabled)} label={t("settings.system.trashEnabled")} />
                </SettingRow>
                <SettingRow label={t("settings.system.trashRetention")}>
                  <input type="number" min={1} value={settings.trashRetentionDays ?? 30} onChange={e => onSetSetting("trashRetentionDays", Math.max(1, Number(e.target.value) || 30))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "var(--line-muted)" }} />
                </SettingRow>
              </SettingsSection>

              <SettingsSection title={t("settings.system.editorAndHistory")}>
                <SettingRow label={t("settings.system.autosaveDelay")}>
                  <input type="number" min={100} step={50} value={settings.autosaveDelay ?? 400} onChange={e => onSetSetting("autosaveDelay", Math.max(100, Number(e.target.value) || 400))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "var(--line-muted)" }} />
                </SettingRow>
                <SettingRow label={t("settings.system.revisionLimit")}>
                  <input type="number" min={1} value={settings.revisionLimit ?? 50} onChange={e => onSetSetting("revisionLimit", Math.max(1, Number(e.target.value) || 50))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "var(--line-muted)" }} />
                </SettingRow>
                <SettingRow label={t("settings.system.startView")}>
                  <div className="w-56"><Select value={settings.startView ?? "lastNote"} options={START_VIEWS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("startView", v as import("../hooks/useSettings").StartView)} /></div>
                </SettingRow>
              </SettingsSection>

              <SettingsSection title={t("update.title")}>
                {!isMobilePlatform && <UpdateChecker settings={settings} onSetSetting={onSetSetting} />}
                <SettingRow label={t("settings.whatsNewOnUpdate")}>
                  <Toggle
                    checked={settings.whatsNewOnUpdate}
                    onChange={() => onSetSetting("whatsNewOnUpdate", !settings.whatsNewOnUpdate)}
                    label={t("settings.whatsNewOnUpdate")}
                  />
                </SettingRow>
              </SettingsSection>
            </SettingsGrid>
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
              <SettingsSection title={t("settings.stats.sections.overview")}>
                <dl className="grid grid-cols-2 gap-4 text-gray-800">
                  <div><dt className="text-xs text-gray-500">{t("settings.stats.notes")}</dt><dd className="text-2xl font-bold">{stats.notes}</dd></div>
                  <div><dt className="text-xs text-gray-500">{t("settings.stats.archived")}</dt><dd className="text-2xl font-bold">{stats.archived}</dd></div>
                  <div><dt className="text-xs text-gray-500">{t("settings.stats.characters")}</dt><dd className="text-2xl font-bold">{stats.characters}</dd></div>
                  <div><dt className="text-xs text-gray-500">{t("settings.stats.words")}</dt><dd className="text-2xl font-bold">{stats.words}</dd></div>
                </dl>
              </SettingsSection>
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
  | { mode: "vault"; c: ContextInfo }
  | { mode: "inviteShare"; c: ContextInfo }
  | { mode: "inviteAccept"; c: ContextInfo }
  | { mode: "rotate"; c: ContextInfo }
  | null;

function ContextsPage() {
  const { t } = useTranslation();
  const vault = useVault();
  const [ctx, setCtx] = useState<ContextInfo[]>([]);
  const [dialog, setDialog] = useState<CtxDialog>(null);
  const [deleteFile, setDeleteFile] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [inviteCode, setInviteCode] = useState<string | null>(null);
  const [rotationCodes, setRotationCodes] = useState<RotationCode[] | null>(null);
  const [inviteCodes, setInviteCodes] = useState<InviteCode[] | null>(null);
  const [sharing, setSharing] = useState(false);
  // Kept apart from `error` (the add-a-context flow's, shown at the bottom):
  // a failed share belongs next to the row whose button started it.
  const [shareError, setShareError] = useState<string | null>(null);
  // One open actions menu at a time, anchored below the row's trigger.
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null);

  // The invite commands act on the ACTIVE server context (they resolve their
  // workspace and tokens from it), so they are only offered on that row —
  // never on some other context the backend would not be talking to.
  const canInvite = (c: ContextInfo) => c.active && c.kind === "server" && c.workspaceId !== "";

  /**
   * Sharing wraps the ring's newest DEK under a one-time code, so the vault
   * has to be open AND that DEK has to be the workspace's current key: a
   * conflicted device may still seal with its own vault's key, and a device
   * that has not redeemed the latest rotation holds a retired one — the
   * backend refuses both (`invite_wrap_allowed`), and by then the user has
   * already pasted an invitation link.
   */
  const canShare = (c: ContextInfo) =>
    canInvite(c) && c.vaultExists && vault.status.unlocked && !vault.status.conflict && !vault.status.sealOutdated;

  /**
   * Rotating needs everything sharing needs, plus a workspace that is
   * actually asking for it: the backend refuses with "no rotation pending"
   * otherwise, and the new key is minted here and installed into the live
   * ring, which a locked vault has no place for.
   */
  const canRotate = (c: ContextInfo) => canShare(c) && c.vaultRotationPending;

  /** Why a vault action is withheld on this row, if it is — first reason wins. */
  const shareBlockedHint = (c: ContextInfo): string | null => {
    if (!(canInvite(c) && c.vaultExists)) return null;
    if (!vault.status.unlocked) return t("vault.lockedHint");
    if (vault.status.conflict) return t("vault.invite.conflictHint");
    if (vault.status.sealOutdated) return t("vault.invite.outdatedHint");
    return null;
  };

  /**
   * Maps a failed vault-mint attempt (share or re-code — both take the DEK
   * out of the live ring and can only fail the same three ways) to the
   * message to show next to the row. A locked vault and a stale context are
   * the two failures with an obvious next step, so neither goes through the
   * raw-text interpolation.
   */
  const shareErrorFor = (e: unknown) => {
    const msg = e instanceof Error ? e.message : String(e ?? "");
    return msg.includes("vault locked") ? t("vault.lockedHint")
      : msg.includes("context changed during the request") ? t("common.contextChanged")
      : msg.includes("resolve the vault conflict first") ? t("vault.invite.conflictHint")
      : msg.includes("redeem the rotation code first") ? t("vault.invite.outdatedHint")
      : t("vault.invite.shareFailed", { error: msg });
  };

  /**
   * Resolve whatever was pasted into an invitation id, then attach the key.
   * The two halves fail for unrelated reasons — a link nobody can resolve on
   * the one hand, and a closed invitation, a wrap already attached, a rotated
   * generation or a locked vault on the other — so they are reported apart,
   * and the attach failure carries the backend's own words.
   */
  const shareVault = async (reference: string) => {
    close();
    setShareError(null);
    setSharing(true);
    try {
      let id: number;
      try {
        id = await api.contexts.vaultInviteResolve(reference);
      } catch {
        setShareError(t("vault.invite.resolveFailed"));
        return;
      }
      try {
        setInviteCode(await api.contexts.vaultInviteShare(id));
      } catch (e) {
        setShareError(shareErrorFor(e));
      }
    } finally {
      setSharing(false);
    }
  };

  /**
   * Mints a fresh code for every open invitation whose wrap a key rotation
   * retired — the same failure mapping as `shareVault`, since both mint a
   * code from the live ring and can only fail the same three ways.
   */
  const recode = async () => {
    if (sharing) return;
    setShareError(null);
    setSharing(true);
    try {
      setInviteCodes(await api.vault.inviteRecode());
      setCtx(await api.contexts.list());
    } catch (e) {
      setShareError(shareErrorFor(e));
    } finally {
      setSharing(false);
    }
  };

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
      <SettingsSection title={t("contexts.sections.manage")}>
        {ctx.map(c => (
          <div key={c.id} data-testid={`context-row-${c.id}`} className="flex items-start justify-between gap-3 rounded border px-3 py-2" style={{ borderColor: "var(--line-muted)", background: "var(--paper-raised)" }}>
            <div className="min-w-0">
              <div className="flex items-center gap-2 text-sm font-medium text-gray-800">
                {c.kind === "server" && <FontAwesomeIcon icon={faGlobe} className="text-[11px] text-gray-500 shrink-0" />}
                <span className="truncate">{labelOf(c)}</span>
                {c.active && (
                  <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide" style={{ background: "var(--line)", color: "#1c1917" }}>{t("contexts.active")}</span>
                )}
              </div>
              <div className="text-xs text-gray-500 break-all font-mono">{c.kind === "server" ? c.serverUrl : c.path}</div>
              <div className="mt-1 flex items-center gap-1.5 flex-wrap">
                <span className="rounded px-1.5 py-0.5 text-[10px] font-medium" style={{ background: "var(--line-muted)", color: "#1c1917" }}>
                  {c.vaultExists ? t("contexts.vault.set") : t("contexts.vault.none")}
                </span>
                {c.vaultExists && c.vaultBiometric && (
                  <span className="rounded px-1.5 py-0.5 text-[10px] font-medium" style={{ background: "var(--line-muted)", color: "#1c1917" }}>
                    {t("contexts.vault.touchId")}
                  </span>
                )}
                {c.vaultGeneration > 1 && (
                  <span className="rounded px-1.5 py-0.5 text-[10px] font-medium" style={{ background: "var(--line-muted)", color: "#1c1917" }}>
                    {t("contexts.vault.generation", { n: c.vaultGeneration })}
                  </span>
                )}
                {c.vaultRotationPending && (
                  <span className="rounded px-1.5 py-0.5 text-[10px] font-medium" style={{ background: "#fef3c7", color: "#7c2d12" }}>
                    {t("contexts.vault.rotationPending")}
                  </span>
                )}
                {c.invitesNeedingCode > 0 && (
                  <span className="rounded px-1.5 py-0.5 text-[10px] font-medium" style={{ background: "#fef3c7", color: "#7c2d12" }}>
                    {t("contexts.vault.invitesNeedCode", { count: c.invitesNeedingCode })}
                  </span>
                )}
              </div>
            </div>
            {/* Every row action lives in one menu: six buttons never fit a
                phone-width row, and the menu only lists what applies here. */}
            <div className="flex shrink-0 flex-col items-end gap-1">
              <button
                onClick={e => {
                  // Keep the click away from the open menu's dismiss listener,
                  // so switching rows opens the new menu instead of closing it.
                  e.stopPropagation();
                  const r = e.currentTarget.getBoundingClientRect();
                  setMenu(m => (m?.id === c.id ? null : { id: c.id, x: r.left, y: r.bottom + 4 }));
                }}
                aria-haspopup="menu"
                aria-expanded={menu?.id === c.id}
                className="px-3 py-1 rounded text-xs font-medium border"
                style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}
              >
                {t("contexts.actions")} ▾
              </button>
              {menu?.id === c.id && (
                <ContextMenu
                  x={menu.x}
                  y={menu.y}
                  onClose={() => setMenu(m => (m?.id === c.id ? null : m))}
                  items={[
                    { label: t("contexts.rename"), onClick: () => setDialog({ mode: "rename", c }) },
                    ...(c.vaultExists ? [{ label: t("contexts.vault.changePassphrase"), onClick: () => setDialog({ mode: "vault", c }) }] : []),
                    // Sharing and rotating both take the DEK out of the live ring,
                    // so both are hidden while the vault is locked — with the hint
                    // below saying why, rather than an item the backend refuses.
                    ...(canShare(c) ? [{ label: t("vault.invite.share"), onClick: () => { setShareError(null); setDialog({ mode: "inviteShare", c }); } }] : []),
                    ...(canShare(c) && c.invitesNeedingCode > 0 ? [{ label: t("vault.invite.recode"), onClick: () => void recode() }] : []),
                    ...(canRotate(c) ? [{ label: t("vault.rotation.run"), onClick: () => { setError(null); setDialog({ mode: "rotate", c }); } }] : []),
                    ...(canInvite(c) ? [{ label: t("vault.invite.enter"), onClick: () => { setError(null); setDialog({ mode: "inviteAccept", c }); } }] : []),
                    { label: t("contexts.remove"), disabled: c.active, onClick: () => setDialog({ mode: "remove", c }) },
                  ]}
                />
              )}
              {shareBlockedHint(c) && <span className="text-xs text-gray-500 text-right">{shareBlockedHint(c)}</span>}
              {sharing && c.active && <span className="text-xs text-gray-500">{t("contexts.connecting")}</span>}
              {shareError && c.active && <span className="text-xs text-red-600 text-right" role="alert">{shareError}</span>}
            </div>
          </div>
        ))}
        <div className="flex items-center gap-2">
          <button onClick={() => setDialog({ mode: "add" })} className="px-4 py-1.5 rounded text-sm font-medium" style={{ background: "var(--line)", color: "#1c1917" }}>{t("contexts.add")}</button>
          <button onClick={() => { setError(null); setDialog({ mode: "addServer" }); }} className="px-4 py-1.5 rounded text-sm font-medium border" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>{t("contexts.addServer")}</button>
          {connecting && <span className="text-xs text-gray-500">{t("contexts.connecting")}</span>}
          {error && <span className="text-xs text-red-600" role="alert">{error}</span>}
        </div>
      </SettingsSection>

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
      {dialog?.mode === "inviteShare" && (
        <PromptDialog
          title={t("vault.invite.share")}
          confirmLabel={t("vault.invite.share")}
          placeholder={t("vault.invite.reference")}
          hint={t("vault.invite.shareHintDetail")}
          onSubmit={reference => void shareVault(reference)}
          onCancel={close}
        />
      )}
      {inviteCode && <VaultInviteCodeDialog code={inviteCode} onClose={() => setInviteCode(null)} />}
      {dialog?.mode === "rotate" && (
        <VaultRotateDialog
          recoveryHolder={vault.status.recoveryHolder}
          rotate={(passphrase, recoveryKey) => api.vault.rotate(passphrase, recoveryKey)}
          onSuccess={codes => { close(); setRotationCodes(codes); void api.contexts.list().then(setCtx); }}
          onCancel={close}
        />
      )}
      {rotationCodes && (
        <VaultCodesDialog
          title={t("vault.rotation.codesTitle")}
          hint={t("vault.rotation.codesHint")}
          entries={rotationCodes.map(c => ({ id: `member-${c.userId}`, label: c.name.trim() ? c.name : t("vault.rotation.codeFor", { id: c.userId }), code: c.code }))}
          onClose={() => setRotationCodes(null)}
        />
      )}
      {inviteCodes && (
        <VaultCodesDialog
          title={t("vault.invite.recodeTitle")}
          hint={t("vault.invite.recodeHint")}
          entries={inviteCodes.map(c => ({ id: `inv-${c.invitationId}`, label: t("vault.invite.codeForInvitation", { id: c.invitationId }), code: c.code }))}
          onClose={() => setInviteCodes(null)}
        />
      )}
      {dialog?.mode === "inviteAccept" && (
        <VaultAcceptInviteDialog
          resolve={api.contexts.vaultInviteResolve}
          accept={api.contexts.vaultInviteAccept}
          onSuccess={() => { close(); void api.contexts.list().then(setCtx); }}
          onCancel={close}
        />
      )}
      {dialog?.mode === "vault" && (
        <ChangePassphraseDialog
          title={t("contexts.vault.changePassphrase")}
          onSubmit={(current, next) => api.contexts.vaultChangePassphrase(dialog.c.id, current, next)}
          onClose={() => { close(); void api.contexts.list().then(setCtx); }}
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
      <SettingsSection title={t("diagnostics.sections.checks")}>
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
        <button onClick={run} className="self-start px-4 py-1.5 rounded text-sm font-medium border" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>{t("diagnostics.recheck")}</button>
      </SettingsSection>
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

  const [tab, setTab] = useState<"server" | "access">("server");
  const tabs = [
    { id: "server", label: t("settings.mcp.tabs.server") },
    { id: "access", label: t("settings.mcp.tabs.access") },
  ];

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t("settings.mcp.title")}</h1>
      <p className="text-sm text-gray-500 mb-6">{t("settings.mcp.subtitle")}</p>

      <SettingsTabs tabs={tabs} active={tab} onChange={id => setTab(id as "server" | "access")} />

      {tab === "server" && (
        <SettingsGrid>
          <SettingsSection title={t("settings.mcp.sections.config")}>
            <SettingRow label={t("settings.mcp.enabled")}>
              <Toggle checked={settings.mcpEnabled ?? false} onChange={() => onSetSetting("mcpEnabled", !settings.mcpEnabled)} label={t("settings.mcp.enabled")} />
            </SettingRow>

            <SettingRow label={t("settings.mcp.reachable")}>
              <div className="w-40"><Select value={settings.mcpBind ?? "internal"} options={MCP_BINDS.map(o => ({ value: o.value, label: t(o.labelKey) }))} onChange={v => onSetSetting("mcpBind", v as "internal" | "external")} /></div>
            </SettingRow>
            {settings.mcpBind === "external" && (
              <div className="rounded border px-3 py-2 text-xs" style={{ background: "#fee2e2", borderColor: "#fca5a5", color: "#991b1b" }}>
                {t("settings.mcp.externalWarning")}
              </div>
            )}

            <SettingRow label={t("settings.mcp.port")}>
              <input type="number" min={1} max={65535} value={settings.mcpPort ?? 4357} onChange={e => onSetSetting("mcpPort", Math.min(65535, Math.max(1, Number(e.target.value) || 4357)))} className="w-24 bg-white border rounded px-2 py-1" style={{ borderColor: "var(--line-muted)" }} />
            </SettingRow>

            <SettingRow label={t("settings.mcp.authRequired")}>
              <Toggle checked={settings.mcpAuthRequired ?? true} onChange={() => onSetSetting("mcpAuthRequired", !settings.mcpAuthRequired)} label={t("settings.mcp.authRequired")} />
            </SettingRow>

            <SettingRow label={t("settings.mcp.token")} stack>
              <div className="flex items-center gap-2">
                <input type="text" readOnly value={settings.mcpToken} className="flex-1 bg-white border rounded px-2 py-1 text-xs font-mono" style={{ borderColor: "var(--line-muted)" }} />
                <button onClick={() => onSetSetting("mcpToken", crypto.randomUUID())} className="shrink-0 px-3 py-1 rounded text-xs font-medium border" style={{ borderColor: "var(--line-muted)", color: "#1c1917" }}>
                  {t("settings.mcp.regenerate")}
                </button>
              </div>
            </SettingRow>
          </SettingsSection>

          <div className="md:col-span-2">
            <SettingsSection title={t("settings.mcp.sections.connection")}>
              <SettingRow label={t("settings.mcp.status")} stack>
                <p className="text-xs text-gray-600 break-all font-mono">{url}</p>
              </SettingRow>

              <SettingRow label={t("settings.mcp.demo")} hint={t("settings.mcp.demoHint")} stack>
                <div className="relative">
                  <pre className="bg-white border rounded p-3 text-[11px] leading-relaxed font-mono overflow-auto" style={{ borderColor: "var(--line-muted)" }}>{demo}</pre>
                  <button onClick={copyDemo} className="absolute top-2 right-2 px-2 py-0.5 rounded text-[11px] font-medium border" style={{ background: "var(--line)", borderColor: "var(--line-muted)", color: "#1c1917" }}>
                    {copied ? t("settings.mcp.copied") : t("settings.mcp.copy")}
                  </button>
                </div>
              </SettingRow>
            </SettingsSection>
          </div>
        </SettingsGrid>
      )}

      {tab === "access" && (
        <SettingsGrid>
          <SettingsSection title={t("settings.mcp.tabs.access")}>
            <SettingRow label={t("settings.mcp.allowWrite")}>
              <Toggle checked={settings.mcpAllowWrite ?? false} onChange={() => onSetSetting("mcpAllowWrite", !settings.mcpAllowWrite)} label={t("settings.mcp.allowWrite")} />
            </SettingRow>

            <SettingRow label={t("settings.mcp.protectedAccess")}>
              <div className="w-40">
                <Select
                  value={settings.mcpProtectedAccess ?? "off"}
                  options={MCP_PROTECTED_ACCESS.map(o => ({ value: o.value, label: t(o.labelKey) }))}
                  onChange={v => onSetSetting("mcpProtectedAccess", v as import("../hooks/useSettings").McpProtectedAccess)}
                />
              </div>
            </SettingRow>
            {settings.mcpProtectedAccess !== "off" && (
              <div className="rounded border px-3 py-2 text-xs" style={{ background: "#fee2e2", borderColor: "#fca5a5", color: "#991b1b" }}>
                {t("settings.mcp.protectedAccessWarning")}
              </div>
            )}
          </SettingsSection>
        </SettingsGrid>
      )}
    </div>
  );
}
