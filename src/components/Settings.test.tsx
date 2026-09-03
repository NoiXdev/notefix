import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

const {
  mockIsEnabled,
  mockEnable,
  mockDisable,
  mockGetDbPath,
  mockSetDbLocation,
  mockRelaunch,
  mockPickFolder,
  mockVaultStatus,
  mockVaultSetup,
  mockVaultUnlock,
  mockVaultUnlockRecovery,
  mockVaultUnlockBiometric,
  mockVaultLock,
  mockVaultChangePassphrase,
  mockVaultRotate,
  mockRotationRedeem,
  mockRecoveryFollowup,
  mockRecoveryCreate,
  mockResolveConflict,
  mockBiometricAvailable,
  mockBiometricEnable,
  mockBiometricDisable,
  mockCheckForUpdate,
  mockCheckPaths,
  mockWindowProbe,
  mockContextsList,
  mockContextsAdd,
  mockContextsRename,
  mockContextsRemove,
  mockContextsVaultChangePassphrase,
  mockInviteResolve,
  mockInviteShare,
  mockInviteAccept,
  mockInviteRecode,
  mockServerAuthBegin,
  mockOnContextChanged,
  mockOpenExternal,
  mockUseIsMobile,
} = vi.hoisted(() => ({
  mockIsEnabled: vi.fn(() => Promise.resolve(false)),
  mockEnable: vi.fn(() => Promise.resolve()),
  mockDisable: vi.fn(() => Promise.resolve()),
  mockGetDbPath: vi.fn(() => Promise.resolve("/data/notefix.db")),
  mockSetDbLocation: vi.fn(() => Promise.resolve({ mode: "moved", path: "/new/notefix.db" })),
  mockRelaunch: vi.fn(),
  mockPickFolder: vi.fn(() => Promise.resolve("/new")),
  mockVaultStatus: vi.fn(() => Promise.resolve({ exists: false, unlocked: false, biometric: false })),
  mockVaultSetup: vi.fn(() => Promise.resolve(["code-1", "code-2"])),
  mockVaultUnlock: vi.fn(() => Promise.resolve()),
  mockVaultUnlockRecovery: vi.fn(() => Promise.resolve()),
  mockVaultUnlockBiometric: vi.fn(() => Promise.resolve()),
  mockVaultLock: vi.fn(() => Promise.resolve()),
  mockVaultChangePassphrase: vi.fn(() => Promise.resolve()),
  mockVaultRotate: vi.fn((_passphrase: string, _recoveryKey?: string) =>
    Promise.resolve([{ userId: 2, name: "", code: "AAAAA-BBBBB" }, { userId: 3, name: "", code: "CCCCC-DDDDD" }])),
  mockRotationRedeem: vi.fn((_code: string, _passphrase: string) => Promise.resolve()),
  mockRecoveryFollowup: vi.fn((_recoveryKey: string) => Promise.resolve()),
  mockRecoveryCreate: vi.fn(() => Promise.resolve({ groups: ["AAAAA", "BBBBB", "CCCCC"], incomplete: false })),
  mockResolveConflict: vi.fn(() => Promise.resolve({ changed: 0, skipped: 0 })),
  mockBiometricAvailable: vi.fn(() => Promise.resolve(false)),
  mockBiometricEnable: vi.fn(() => Promise.resolve()),
  mockBiometricDisable: vi.fn(() => Promise.resolve()),
  mockCheckForUpdate: vi.fn(() => Promise.resolve({ current: "0.7.0", latest: "0.7.0", updateAvailable: false, url: "" })),
  mockCheckPaths: vi.fn(() => Promise.resolve({ dbWritable: true, imagesWritable: true, dbPath: "/data/notefix.db", imagesPath: "/data/images" })),
  mockWindowProbe: vi.fn(() => Promise.resolve(true)),
  mockContextsList: vi.fn(() => Promise.resolve([])),
  mockContextsAdd: vi.fn(() => Promise.resolve([])),
  mockContextsRename: vi.fn(() => Promise.resolve([])),
  mockContextsRemove: vi.fn(() => Promise.resolve([])),
  mockContextsVaultChangePassphrase: vi.fn(() => Promise.resolve()),
  mockInviteResolve: vi.fn((_reference: string) => Promise.resolve(7)),
  mockInviteShare: vi.fn((_id: number) => Promise.resolve("ABCDE-FGHJK-MNPQR")),
  mockInviteAccept: vi.fn((_id: number, _code: string, _passphrase: string) => Promise.resolve()),
  mockInviteRecode: vi.fn(() => Promise.resolve([])),
  mockServerAuthBegin: vi.fn(() => Promise.resolve("https://server.example.com/authorize")),
  mockOnContextChanged: vi.fn(() => () => {}),
  mockOpenExternal: vi.fn(),
  mockUseIsMobile: vi.fn(() => false),
}));

vi.mock("../api", () => ({
  api: {
    getAppInfo: vi.fn(() => Promise.resolve({ name: "Notefix", version: "0.1.0", description: "x" })),
    autostart: { isEnabled: mockIsEnabled, enable: mockEnable, disable: mockDisable },
    stats: vi.fn(() => Promise.resolve({ notes: 3, archived: 1, characters: 42, words: 8 })),
    githubReleases: vi.fn(() => Promise.resolve([])),
    openExternal: mockOpenExternal,
    getDbPath: mockGetDbPath,
    setDbLocation: mockSetDbLocation,
    relaunch: mockRelaunch,
    pickFolder: mockPickFolder,
    checkForUpdate: mockCheckForUpdate,
    checkPaths: mockCheckPaths,
    windowProbe: mockWindowProbe,
    onContextChanged: mockOnContextChanged,
    vault: {
      status: mockVaultStatus,
      setup: mockVaultSetup,
      unlock: mockVaultUnlock,
      unlockRecovery: mockVaultUnlockRecovery,
      unlockBiometric: mockVaultUnlockBiometric,
      lock: mockVaultLock,
      changePassphrase: mockVaultChangePassphrase,
      rotate: mockVaultRotate,
      rotationRedeem: mockRotationRedeem,
      recoveryFollowup: mockRecoveryFollowup,
      recoveryCreate: mockRecoveryCreate,
      resolveConflict: mockResolveConflict,
      biometricAvailable: mockBiometricAvailable,
      biometricEnable: mockBiometricEnable,
      biometricDisable: mockBiometricDisable,
      inviteRecode: mockInviteRecode,
    },
    contexts: {
      list: mockContextsList,
      add: mockContextsAdd,
      rename: mockContextsRename,
      remove: mockContextsRemove,
      vaultChangePassphrase: mockContextsVaultChangePassphrase,
      vaultInviteResolve: mockInviteResolve,
      vaultInviteShare: mockInviteShare,
      vaultInviteAccept: mockInviteAccept,
      serverAuthBegin: mockServerAuthBegin,
    },
  },
}));
vi.mock('react-select', () => ({
  default: ({ options, value, onChange }: { options: { value: string; label: string }[]; value: { value: string } | null; onChange: (o: { value: string }) => void }) => (
    <select aria-label="select" value={value?.value ?? ''} onChange={e => onChange(options.find(o => o.value === e.target.value)!)}>
      {options.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
    </select>
  ),
}));
vi.mock("../hooks/useIsMobile", () => ({ useIsMobile: mockUseIsMobile }));

// `isMobilePlatform` is a plain const computed once from the UA at module
// load, so it can't be flipped by changing navigator.userAgent after the
// fact. Mock the module with a getter instead, so each render re-reads the
// current `platformState.isMobilePlatform` — same live-binding trick Vitest
// docs use for "constants" that need to vary between tests.
const platformState = vi.hoisted(() => ({ isMobilePlatform: false }));
vi.mock("../platform", () => ({
  get isMobilePlatform() { return platformState.isMobilePlatform; },
}));

import Settings from "./Settings";
import { api } from "../api";
import type { AppSettings } from "../hooks/useSettings";
import type { VaultStatus } from "../types";

/** A COMPLETE `VaultStatus`, so a test only has to spell out what it varies. */
const vaultStatus = (overrides: Partial<VaultStatus> = {}): VaultStatus => ({
  exists: false, unlocked: false, biometric: false, conflict: false,
  recoveryHolder: true, rotationCode: false, recoveryMissing: false, sealOutdated: false,
  recoveryEligible: false,
  ...overrides,
});

/** An unlocked vault on a workspace — what every vault ACTION needs. */
const unlockedVault = (overrides: Partial<VaultStatus> = {}) =>
  vaultStatus({ exists: true, unlocked: true, ...overrides });

beforeEach(() => {
  vi.clearAllMocks();
  mockUseIsMobile.mockReturnValue(false);
  platformState.isMobilePlatform = false;
  mockVaultStatus.mockResolvedValue(vaultStatus());
});

/** Every AppSettings field at its default (mirrors useSettings.ts DEFAULTS), so new tests don't need to hand-roll the whole shape. */
const FULL_SETTINGS: AppSettings = {
  startMinimized: false,
  dateFormat: "auto",
  pinnedScope: "perFolder",
  folderColorStyle: "icon",
  revisionLimit: 50,
  autosaveDelay: 400,
  startView: "lastNote",
  sidebarMode: "switcher",
  dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }],
  compactTree: false,
  treeProgress: true,
  trashEnabled: true,
  trashRetentionDays: 30,
  closeAction: "ask",
  shortcuts: {},
  language: "system",
  linkPreviewEnabled: true,
  linkPreviewMode: "card",
  copyFormat: "md",
  mcpEnabled: false,
  mcpBind: "internal",
  mcpPort: 4357,
  mcpAuthRequired: true,
  mcpToken: "",
  mcpAllowWrite: false,
  mcpProtectedAccess: "off",
  checkUpdatesOnStart: true,
  updateDismissedVersion: "",
  lastSeenVersion: "",
  whatsNewOnUpdate: true,
  searchScope: "context",
  theme: "butter",
  editorCountShow: true,
  editorCountPos: "topRight",
  editorInvisibles: false,
  editorLineHeight: "normal",
  editorToolbarPos: "bottom",
  editorFontSize: "medium",
  editorFontFamily: "sans",
  editorWidth: "full",
  sidebarSide: "left",
  autoLockIdle: true,
  autoLockOnHide: true,
  autoLockMinutes: 5,
  autoLockOnSleep: true,
  vaultBiometric: false,
  vaultLockScope: "session",
};

describe("Settings — Darstellung", () => {
  it("shows the About page by default", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder", folderColorStyle: "icon" }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("Notefix")).toBeInTheDocument());
  });

  it("shows the logo on the About page", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder", folderColorStyle: "icon" }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    expect(await screen.findByAltText("Notefix")).toBeInTheDocument();
  });
});

describe("Settings — System", () => {
  it("toggling start-minimized calls onSetSetting", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/Minimiert starten/));
    expect(onSetSetting).toHaveBeenCalledWith("startMinimized", true);
  });

  it("enabling start-on-boot calls autostart.enable", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/Bei Anmeldung starten/));
    expect(mockEnable).toHaveBeenCalledOnce();
  });

  it("'export all' calls onExport with empty ids", () => {
    const onExport = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} onExport={onExport} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Alle als JSON exportieren"));
    expect(onExport).toHaveBeenCalledWith([], "notefix-export.json");
  });
});

describe("Settings — date format & stats", () => {
  it("selecting a date format calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.change(screen.getByDisplayValue("Auto (relativ)"), { target: { value: "iso" } });
    expect(onSetSetting).toHaveBeenCalledWith("dateFormat", "iso");
  });

  it("stats page shows the counts", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Statistik"));
    await waitFor(() => expect(screen.getByText("42")).toBeInTheDocument());
  });
});

describe("Settings — Speicherort", () => {
  it("shows the db path and changes location then offers restart", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    await waitFor(() => expect(screen.getByText("/data/notefix.db")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Ändern…"));
    await waitFor(() => expect(screen.getByText("Jetzt neu starten")).toBeInTheDocument());
    expect(mockSetDbLocation).toHaveBeenCalledWith("/new");
    fireEvent.click(screen.getByText("Jetzt neu starten"));
    expect(mockRelaunch).toHaveBeenCalledOnce();
  });
});

describe("Settings — folderColorStyle", () => {
  it("selecting a folder color style calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Liste & Ordner"));
    fireEvent.change(screen.getByDisplayValue("Nur Icon einfärben"), { target: { value: "row" } });
    expect(onSetSetting).toHaveBeenCalledWith("folderColorStyle", "row");
  });
});

describe("Settings — pinnedScope", () => {
  it("selecting global calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Liste & Ordner"));
    fireEvent.change(screen.getByDisplayValue("Gepinnt zuerst je Ordner"), { target: { value: "global" } });
    expect(onSetSetting).toHaveBeenCalledWith("pinnedScope", "global");
  });
});

describe("Settings — editor & history", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false };
  it("changing the revision limit calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.change(screen.getByDisplayValue("50"), { target: { value: "10" } });
    expect(onSetSetting).toHaveBeenCalledWith("revisionLimit", 10);
  });
});

describe("Settings — tree view", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false };
  it("toggling compact view calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Liste & Ordner"));
    fireEvent.click(screen.getByLabelText(/Kompakte Ansicht/));
    expect(onSetSetting).toHaveBeenCalledWith("compactTree", true);
  });
});

describe("Settings — trash", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false };
  it("toggling trash calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/Papierkorb verwenden/));
    expect(onSetSetting).toHaveBeenCalledWith("trashEnabled", false);
  });
});

describe("Settings — shortcuts page", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false };
  it("lists the new-note shortcut", () => {
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Tastatur"));
    expect(screen.getByText("Neue Notiz")).toBeInTheDocument();
  });
});

describe("Settings — close behavior", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false };
  it("changing close behavior calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.change(screen.getByDisplayValue("Fragen"), { target: { value: "quit" } });
    expect(onSetSetting).toHaveBeenCalledWith("closeAction", "quit");
  });
});

describe("Settings — Security", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false, autoLockIdle: true, autoLockOnHide: true, autoLockMinutes: 5, autoLockOnSleep: true, vaultBiometric: false, vaultLockScope: "session" as const };

  it("renders the auto-lock toggles and toggling 'after inactivity' calls onSetSetting", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    fireEvent.click(screen.getByText("Auto-Lock"));
    await waitFor(() => expect(screen.getByText("Nach Inaktivität sperren")).toBeInTheDocument());
    fireEvent.click(screen.getByLabelText("Nach Inaktivität sperren"));
    expect(onSetSetting).toHaveBeenCalledWith("autoLockIdle", false);
  });

  it("renders the lock-scope select and changing it calls onSetSetting", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByDisplayValue("Für diese Sitzung")).toBeInTheDocument());
    fireEvent.change(screen.getByDisplayValue("Für diese Sitzung"), { target: { value: "perNote" } });
    expect(onSetSetting).toHaveBeenCalledWith("vaultLockScope", "perNote");
  });

  it("hides the biometric row when biometricAvailable resolves false", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false });
    mockBiometricAvailable.mockResolvedValueOnce(false);
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    expect(screen.queryByText("Mit Touch ID entsperren")).not.toBeInTheDocument();
  });

  it("shows the biometric toggle when biometricAvailable resolves true and the vault exists", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false });
    mockBiometricAvailable.mockResolvedValueOnce(true);
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Mit Touch ID entsperren")).toBeInTheDocument());
  });

  it("warns about a vault conflict on the workspace without blocking anything", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false, conflict: true, recoveryHolder: true });
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText(/hatte bereits einen Tresor mit einem anderen Schlüssel/)).toBeInTheDocument());
    // Purely informational: the usual actions stay available.
    expect(screen.getByText("Jetzt sperren")).toBeInTheDocument();
  });

  it("opens the conflict dialog from the banner", async () => {
    mockVaultStatus.mockResolvedValue({ exists: true, unlocked: false, biometric: false, conflict: true, recoveryHolder: true, rotationCode: false, recoveryMissing: false, sealOutdated: false });
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    fireEvent.click(await screen.findByRole("button", { name: "Konflikt lösen…" }));
    expect(screen.getByPlaceholderText("Passwort des Arbeitsbereichs")).toBeInTheDocument();
  });

  it("shows no conflict warning when the workspace and this device hold one vault", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false, conflict: false, recoveryHolder: true });
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    expect(screen.queryByText(/hatte bereits einen Tresor/)).not.toBeInTheDocument();
  });

  it("keeps a waiting rotation code reachable from the security page", async () => {
    // The one surface that survives a Touch ID unlock and a postponed step.
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: true, biometric: false, conflict: false,
      recoveryHolder: false, rotationCode: true, recoveryMissing: false,
    });
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText(/Der Tresorschlüssel hat sich geändert/)).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "Wechsel-Code eingeben" }));
    fireEvent.change(screen.getByPlaceholderText("Wechsel-Code"), { target: { value: "AAAA-BBBB" } });
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "member-pw" } });
    fireEvent.click(screen.getByRole("button", { name: "Schlüssel wechseln" }));

    await waitFor(() => expect(mockRotationRedeem).toHaveBeenCalledWith("AAAA-BBBB", "member-pw"));
    await waitFor(() => expect(screen.queryByPlaceholderText("Wechsel-Code")).not.toBeInTheDocument());
  });

  it("does not offer to redeem a rotation code while the vault is locked", async () => {
    // Redeeming re-wraps the new key under the passphrase, so the backend
    // refuses with "vault locked" — offering the button there would report a
    // perfectly good one-time code as burnt.
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: false, biometric: false, conflict: false,
      recoveryHolder: false, rotationCode: true, recoveryMissing: false,
    });
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText(/Der Tresorschlüssel hat sich geändert/)).toBeInTheDocument());

    expect(screen.getByText(/Entsperre zuerst den Tresor/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Wechsel-Code eingeben" })).not.toBeInTheDocument();
    expect(mockRotationRedeem).not.toHaveBeenCalled();
  });

  it("shows no rotation-code banner when nothing is waiting", async () => {
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: true, biometric: false, conflict: false,
      recoveryHolder: true, rotationCode: false, recoveryMissing: false,
    });
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    expect(screen.queryByText(/Der Tresorschlüssel hat sich geändert/)).not.toBeInTheDocument();
  });

  it("lets the recovery-key holder fill in the wrap a foreign key change left out", async () => {
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: true, biometric: false, conflict: false,
      recoveryHolder: true, rotationCode: false, recoveryMissing: true,
    });
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText(/fehlt noch die Hinterlegung/)).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "Wiederherstellungsschlüssel ergänzen" }));
    const keyField = screen.getByPlaceholderText("Wiederherstellungs-Schlüssel");
    fireEvent.change(keyField, { target: { value: "AAAAA-BBBBB-CCCCC" } });
    fireEvent.keyDown(keyField, { key: "Enter" });

    await waitFor(() => expect(mockRecoveryFollowup).toHaveBeenCalledWith("AAAAA-BBBBB-CCCCC"));
    expect(await screen.findByText("Wiederherstellungsschlüssel aktualisiert")).toBeInTheDocument();
  });

  it("reports a rejected recovery key without claiming anything changed", async () => {
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: true, biometric: false, conflict: false,
      recoveryHolder: true, rotationCode: false, recoveryMissing: true,
    });
    mockRecoveryFollowup.mockRejectedValueOnce(new Error("wrong recovery key"));
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText(/fehlt noch die Hinterlegung/)).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "Wiederherstellungsschlüssel ergänzen" }));
    const keyField = screen.getByPlaceholderText("Wiederherstellungs-Schlüssel");
    fireEvent.change(keyField, { target: { value: "nope" } });
    fireEvent.keyDown(keyField, { key: "Enter" });

    expect(await screen.findByText("Falscher Wiederherstellungs-Schlüssel")).toBeInTheDocument();
  });

  it("shows no recovery-wrap hint when every generation has one", async () => {
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: true, biometric: false, conflict: false,
      recoveryHolder: true, rotationCode: false, recoveryMissing: false,
    });
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    expect(screen.queryByText(/fehlt noch die Hinterlegung/)).not.toBeInTheDocument();
  });

  it("offers an owner without a recovery key the chance to create one", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault({ recoveryHolder: false, recoveryEligible: true }));
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() =>
      expect(
        screen.getByText("Du hast noch keinen Wiederherstellungs-Schlüssel für diesen Arbeitsbereich."),
      ).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole("button", { name: "Schlüssel erzeugen" }));
    await waitFor(() => expect(mockRecoveryCreate).toHaveBeenCalledOnce());
    expect(await screen.findByText("AAAAA-BBBBB-CCCCC")).toBeInTheDocument();
  });

  it("ignores a second click on the create-recovery-key button while the first is still in flight", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault({ recoveryHolder: false, recoveryEligible: true }));
    let release: (value: { groups: string[]; incomplete: boolean }) => void = () => {};
    mockRecoveryCreate.mockImplementationOnce(
      () => new Promise(resolve => { release = resolve; }),
    );
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Schlüssel erzeugen" })).toBeInTheDocument(),
    );

    const button = screen.getByRole("button", { name: "Schlüssel erzeugen" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(mockRecoveryCreate).toHaveBeenCalledOnce();

    release({ groups: ["AAAAA", "BBBBB", "CCCCC"], incomplete: false });
    expect(await screen.findByText("AAAAA-BBBBB-CCCCC")).toBeInTheDocument();
    expect(mockRecoveryCreate).toHaveBeenCalledOnce();
  });

  it("shows no recovery-key-creation hint when the caller is not an eligible owner", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault({ recoveryHolder: true, recoveryEligible: false }));
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    expect(screen.queryByText(/keinen Wiederherstellungs-Schlüssel/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Schlüssel erzeugen" })).not.toBeInTheDocument();
  });

  it("offers the recovery key only to a recovery holder", async () => {
    // An invited member holds a wrapped key but no recovery key.
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: false, biometric: false, conflict: false, recoveryHolder: false });
    mockBiometricAvailable.mockResolvedValueOnce(true);
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByRole("switch", { name: "Mit Touch ID entsperren" })).toBeInTheDocument());
    fireEvent.click(screen.getByRole("switch", { name: "Mit Touch ID entsperren" }));

    await waitFor(() => expect(screen.getByText("Tresor entsperren")).toBeInTheDocument());
    expect(screen.queryByText("Wiederherstellungs-Schlüssel verwenden")).not.toBeInTheDocument();
  });

  it("keeps the recovery key for a holder", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: false, biometric: false, conflict: false, recoveryHolder: true });
    mockBiometricAvailable.mockResolvedValueOnce(true);
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByRole("switch", { name: "Mit Touch ID entsperren" })).toBeInTheDocument());
    fireEvent.click(screen.getByRole("switch", { name: "Mit Touch ID entsperren" }));

    await waitFor(() => expect(screen.getByText("Wiederherstellungs-Schlüssel verwenden")).toBeInTheDocument());
  });

  it("names the active context this page manages, and hints that other contexts live under Contexts", async () => {
    mockContextsList.mockResolvedValueOnce([
      { id: "c-local", label: "", kind: "local" as const, path: "/local.db", serverUrl: "", workspaceId: "", active: false, vaultExists: false, vaultBiometric: false },
      { id: "c-server", label: "Team", kind: "server" as const, path: "", serverUrl: "https://s.example.com", workspaceId: "w1", active: true, vaultExists: true, vaultBiometric: false },
    ]);
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Für Team")).toBeInTheDocument());
    expect(screen.getByText("Tresore anderer Kontexte werden unter Kontexte verwaltet.")).toBeInTheDocument();
  });
});

describe("Settings — What's New (About page)", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const, shortcuts: {}, language: "system" as const, linkPreviewEnabled: true, linkPreviewMode: "card" as const, copyFormat: "md" as const, mcpEnabled: false, mcpBind: "internal" as const, mcpPort: 4357, mcpAuthRequired: true, mcpToken: "", mcpAllowWrite: false, whatsNewOnUpdate: true };

  it("fetches releases and opens the dialog when the link is clicked", async () => {
    vi.mocked(api.githubReleases).mockResolvedValueOnce([
      { tagName: "v0.6.0", name: "v0.6.0 — Apps page", body: "Cool stuff", publishedAt: "2026-08-20T00:00:00Z", prerelease: false },
    ]);
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("Notefix")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Neuigkeiten"));

    await waitFor(() => expect(screen.getByText("Neu in dieser Version")).toBeInTheDocument());
    expect(screen.getByText("v0.6.0 — Apps page")).toBeInTheDocument();

    fireEvent.click(screen.getByTitle("Schließen"));
    expect(screen.queryByText("Neu in dieser Version")).not.toBeInTheDocument();
  });

  it("shows an inline error when the fetch fails", async () => {
    vi.mocked(api.githubReleases).mockRejectedValueOnce(new Error("network down"));
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("Notefix")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Neuigkeiten"));

    await waitFor(() => expect(screen.getByText("Änderungsprotokoll konnte nicht geladen werden")).toBeInTheDocument());
    expect(screen.queryByText("Neu in dieser Version")).not.toBeInTheDocument();
  });

  it("toggling 'What's New after update' calls onSetSetting", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/nach einem Update anzeigen/));
    expect(onSetSetting).toHaveBeenCalledWith("whatsNewOnUpdate", false);
  });
});

describe("Settings — navigation & close", () => {
  it("calls onClose when the close button is clicked", () => {
    const onClose = vi.fn();
    render(<Settings onClose={onClose} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByTitle("Zurück zu den Notizen"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("opens directly on the requested initialPage", async () => {
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} initialPage="security" />);
    await waitFor(() => expect(screen.getByText("Notizen mit einem Passwort-Tresor schützen.")).toBeInTheDocument());
  });

  it("hides the MCP nav item when isMobilePlatform is true", () => {
    platformState.isMobilePlatform = true;
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    expect(screen.queryByText("MCP")).not.toBeInTheDocument();
  });

  it("mobile: shows the nav list first; picking a page drills in and the back button returns to the list", async () => {
    mockUseIsMobile.mockReturnValue(true);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    // Nav list is shown; no page content has been drilled into yet.
    expect(screen.getByText("System")).toBeInTheDocument();
    expect(screen.queryByText("Statistik")).toBeInTheDocument(); // still the nav item, not the page

    fireEvent.click(screen.getByText("System"));
    await waitFor(() => expect(screen.getByText("Start- und Hintergrund-Verhalten.")).toBeInTheDocument());
    expect(screen.queryByText("Statistik")).not.toBeInTheDocument(); // nav list is gone now

    fireEvent.click(screen.getByText("Einstellungen")); // back button
    expect(screen.getByText("Statistik")).toBeInTheDocument(); // nav list is back
  });

  it("mobile: initialPage skips the nav list and shows the back button immediately", async () => {
    mockUseIsMobile.mockReturnValue(true);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} initialPage="security" />);
    await waitFor(() => expect(screen.getByText("Notizen mit einem Passwort-Tresor schützen.")).toBeInTheDocument());
    expect(screen.getByText("Einstellungen")).toBeInTheDocument(); // back button, only one match if the nav aside is absent
  });
});

describe("Settings — System page (desktop-only gating)", () => {
  it("hides the start-on-boot section, the storage-location changer and the update checker when isMobilePlatform is true", () => {
    platformState.isMobilePlatform = true;
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    expect(screen.queryByText("Bei Anmeldung starten")).not.toBeInTheDocument();
    expect(screen.queryByText("Ändern…")).not.toBeInTheDocument();
    expect(screen.queryByText("Nach Updates suchen")).not.toBeInTheDocument();
    // Non-gated rows stay visible.
    expect(screen.getByText("Papierkorb verwenden")).toBeInTheDocument();
  });
});

describe("Settings — Update checker (System page)", () => {
  it("checks for updates and shows 'up to date'", async () => {
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Nach Updates suchen"));
    expect(screen.getByText("Suche…")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("Du bist aktuell (0.7.0)")).toBeInTheDocument());
  });

  it("shows an update link and opens it externally when a newer version is available", async () => {
    mockCheckForUpdate.mockResolvedValueOnce({ current: "0.6.0", latest: "0.8.0", updateAvailable: true, url: "https://example.com/dl" });
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Nach Updates suchen"));
    const link = await screen.findByText("Update verfügbar: 0.8.0 — Herunterladen");
    fireEvent.click(link);
    expect(mockOpenExternal).toHaveBeenCalledWith("https://example.com/dl");
  });

  it("shows an error when the update check fails", async () => {
    mockCheckForUpdate.mockRejectedValueOnce(new Error("offline"));
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Nach Updates suchen"));
    await waitFor(() => expect(screen.getByText("Konnte nicht prüfen")).toBeInTheDocument());
  });

  it("toggling 'check on start' calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByRole("switch", { name: "Beim Start nach Updates suchen" }));
    expect(onSetSetting).toHaveBeenCalledWith("checkUpdatesOnStart", false);
  });
});

describe("Settings — Apps page", () => {
  it("lists the platforms and opens the Play Store link for Android", () => {
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Apps"));
    expect(screen.getByText("Android")).toBeInTheDocument();
    expect(screen.getByText("iPhone & iPad")).toBeInTheDocument();
    expect(screen.getByText("Sync-Server")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Im Play Store öffnen"));
    expect(mockOpenExternal).toHaveBeenCalledWith("https://play.google.com/store/apps/details?id=dev.noix.notefix");
  });
});

describe("Settings — Security: vault setup, lock & change passphrase", () => {
  it("sets up the vault, shows the recovery codes, and closes the dialog", async () => {
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Kein Tresor eingerichtet")).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "Tresor einrichten" }));
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "secret123" } });
    fireEvent.change(screen.getByPlaceholderText("Passwort bestätigen"), { target: { value: "secret123" } });
    fireEvent.click(screen.getByRole("button", { name: "Einrichten" }));

    await waitFor(() => expect(mockVaultSetup).toHaveBeenCalledWith("secret123"));
    await waitFor(() => expect(screen.getByText("code-1-code-2")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Ich habe ihn gespeichert"));
    expect(screen.queryByText("Wiederherstellungs-Schlüssel")).not.toBeInTheDocument();
  });

  it("shows 'unlocked' status and lock-now calls vault.lock", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false });
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Jetzt sperren"));
    expect(mockVaultLock).toHaveBeenCalledOnce();
  });

  it("changes the passphrase on the happy path", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false });
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Passwort ändern" }));

    fireEvent.change(screen.getByPlaceholderText("Aktuelles Passwort"), { target: { value: "old" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort"), { target: { value: "new123" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort bestätigen"), { target: { value: "new123" } });
    fireEvent.click(screen.getByRole("button", { name: "Ändern" }));

    await waitFor(() => expect(mockVaultChangePassphrase).toHaveBeenCalledWith("old", "new123"));
    await waitFor(() => expect(screen.queryByPlaceholderText("Aktuelles Passwort")).not.toBeInTheDocument());
  });

  it("shows a mismatch error without calling the API when the new passphrases differ", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false });
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Passwort ändern" }));

    fireEvent.change(screen.getByPlaceholderText("Aktuelles Passwort"), { target: { value: "old" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort"), { target: { value: "new123" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort bestätigen"), { target: { value: "different" } });
    fireEvent.click(screen.getByRole("button", { name: "Ändern" }));

    expect(await screen.findByText("Passwörter stimmen nicht überein")).toBeInTheDocument();
    expect(mockVaultChangePassphrase).not.toHaveBeenCalled();
  });

  it("shows a wrong-current-passphrase error when the API rejects", async () => {
    mockVaultStatus.mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false });
    mockVaultChangePassphrase.mockRejectedValueOnce(new Error("bad current"));
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Entsperrt")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Passwort ändern" }));

    fireEvent.change(screen.getByPlaceholderText("Aktuelles Passwort"), { target: { value: "wrong" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort"), { target: { value: "new123" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort bestätigen"), { target: { value: "new123" } });
    fireEvent.click(screen.getByRole("button", { name: "Ändern" }));

    expect(await screen.findByText("Aktuelles Passwort falsch")).toBeInTheDocument();
  });
});

describe("Settings — Security: biometric unlock", () => {
  it("enables biometric unlock directly when the vault is already unlocked", async () => {
    mockVaultStatus
      .mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false })
      .mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false });
    mockBiometricAvailable.mockResolvedValueOnce(true);
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    const toggle = await screen.findByRole("switch", { name: "Mit Touch ID entsperren" });
    fireEvent.click(toggle);
    await waitFor(() => expect(mockBiometricEnable).toHaveBeenCalledOnce());
    expect(onSetSetting).toHaveBeenCalledWith("vaultBiometric", true);
  });

  it("disables biometric unlock when it is already enabled", async () => {
    mockVaultStatus
      .mockResolvedValueOnce({ exists: true, unlocked: true, biometric: true })
      .mockResolvedValueOnce({ exists: true, unlocked: true, biometric: true });
    mockBiometricAvailable.mockResolvedValueOnce(true);
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    const toggle = await screen.findByRole("switch", { name: "Mit Touch ID entsperren" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    fireEvent.click(toggle);
    await waitFor(() => expect(mockBiometricDisable).toHaveBeenCalledOnce());
    expect(onSetSetting).toHaveBeenCalledWith("vaultBiometric", false);
  });

  it("prompts to unlock before enabling biometric when the vault is locked, then enables it", async () => {
    mockVaultStatus
      .mockResolvedValueOnce({ exists: true, unlocked: false, biometric: false }) // mount
      .mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false }) // after unlock
      .mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false }); // after enableBiometric
    mockBiometricAvailable.mockResolvedValueOnce(true);
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    const toggle = await screen.findByRole("switch", { name: "Mit Touch ID entsperren" });
    fireEvent.click(toggle);

    await waitFor(() => expect(screen.getByText("Tresor entsperren")).toBeInTheDocument());
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "secret123" } });
    fireEvent.click(screen.getByRole("button", { name: "Entsperren" }));

    await waitFor(() => expect(mockVaultUnlock).toHaveBeenCalledWith("secret123"));
    await waitFor(() => expect(mockBiometricEnable).toHaveBeenCalledOnce());
    await waitFor(() => expect(onSetSetting).toHaveBeenCalledWith("vaultBiometric", true));
    expect(screen.queryByText("Tresor entsperren")).not.toBeInTheDocument();
  });
});

describe("Settings — Appearance (general tab)", () => {
  it("picking a theme calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByTitle("Lavendel"));
    expect(onSetSetting).toHaveBeenCalledWith("theme", "lavender");
  });

  it("changing the language calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.change(screen.getByDisplayValue("Automatisch (System)"), { target: { value: "en" } });
    expect(onSetSetting).toHaveBeenCalledWith("language", "en");
  });

  it("changing the copy format calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.change(screen.getByDisplayValue("Markdown"), { target: { value: "html" } });
    expect(onSetSetting).toHaveBeenCalledWith("copyFormat", "html");
  });

  it("toggling link preview calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByRole("switch", { name: "Link-Vorschau" }));
    expect(onSetSetting).toHaveBeenCalledWith("linkPreviewEnabled", false);
  });

  it("changing the link preview mode calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.change(screen.getByDisplayValue("Karte"), { target: { value: "url" } });
    expect(onSetSetting).toHaveBeenCalledWith("linkPreviewMode", "url");
  });
});

describe("Settings — Appearance (list tab)", () => {
  it("changing the sidebar mode calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Liste & Ordner"));
    fireEvent.change(screen.getByDisplayValue("Ein Kontext (Umschalter)"), { target: { value: "combined" } });
    expect(onSetSetting).toHaveBeenCalledWith("sidebarMode", "combined");
  });

  it("changing the sidebar side calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Liste & Ordner"));
    fireEvent.change(screen.getByDisplayValue("Links"), { target: { value: "right" } });
    expect(onSetSetting).toHaveBeenCalledWith("sidebarSide", "right");
  });

  it("toggling tree progress calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Liste & Ordner"));
    fireEvent.click(screen.getByRole("switch", { name: "Fortschritt im Baum zeigen" }));
    expect(onSetSetting).toHaveBeenCalledWith("treeProgress", false);
  });
});

describe("Settings — Appearance (editor tab)", () => {
  it("changing font size, font family, editor width and line height calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Editor"));
    fireEvent.change(screen.getByDisplayValue("Mittel"), { target: { value: "large" } });
    expect(onSetSetting).toHaveBeenCalledWith("editorFontSize", "large");
    fireEvent.change(screen.getByDisplayValue("Sans"), { target: { value: "mono" } });
    expect(onSetSetting).toHaveBeenCalledWith("editorFontFamily", "mono");
    fireEvent.change(screen.getByDisplayValue("Voll"), { target: { value: "narrow" } });
    expect(onSetSetting).toHaveBeenCalledWith("editorWidth", "narrow");
    fireEvent.change(screen.getByDisplayValue("Normal"), { target: { value: "relaxed" } });
    expect(onSetSetting).toHaveBeenCalledWith("editorLineHeight", "relaxed");
  });

  it("changing the toolbar position calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Editor"));
    fireEvent.change(screen.getByDisplayValue("Unten"), { target: { value: "top" } });
    expect(onSetSetting).toHaveBeenCalledWith("editorToolbarPos", "top");
  });

  it("shows the count-position select while char count is on, and toggling it off calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Editor"));
    expect(screen.getByDisplayValue("Oben rechts")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("switch", { name: "Zeichen-/Wortzählung" }));
    expect(onSetSetting).toHaveBeenCalledWith("editorCountShow", false);
  });

  it("hides the count-position select when char count is off", () => {
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, editorCountShow: false }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Editor"));
    expect(screen.queryByDisplayValue("Oben rechts")).not.toBeInTheDocument();
  });

  it("toggling invisibles calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("Editor"));
    fireEvent.click(screen.getByRole("switch", { name: "Sonderzeichen anzeigen" }));
    expect(onSetSetting).toHaveBeenCalledWith("editorInvisibles", true);
  });
});

describe("Settings — Contexts page", () => {
  /** Opens the row's actions menu; every row action lives behind it. */
  const openActions = (id: string) =>
    fireEvent.click(within(screen.getByTestId(`context-row-${id}`)).getByRole("button", { name: "Aktionen ▾" }));

  const contexts = [
    { id: "c-local", label: "", kind: "local" as const, path: "/local.db", serverUrl: "", workspaceId: "", active: true, vaultExists: false, vaultBiometric: false },
    { id: "c-server", label: "Team", kind: "server" as const, path: "", serverUrl: "https://s.example.com", workspaceId: "w1", active: false, vaultExists: true, vaultBiometric: true },
  ];

  it("lists local and server contexts with the active badge", async () => {
    mockContextsList.mockResolvedValueOnce(contexts);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Lokal")).toBeInTheDocument());
    expect(screen.getByText("Team")).toBeInTheDocument();
    expect(screen.getByText("https://s.example.com")).toBeInTheDocument();
    expect(screen.getByText("aktiv")).toBeInTheDocument();
  });

  it("adds a new local context", async () => {
    mockContextsList.mockResolvedValueOnce(contexts);
    mockContextsAdd.mockResolvedValueOnce([...contexts, { id: "c-new", label: "Work", kind: "local" as const, path: "/work.db", serverUrl: "", workspaceId: "", active: false }]);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Lokal")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Kontext hinzufügen…"));
    const input = screen.getByPlaceholderText("Name für die neue lokale Datenbank");
    fireEvent.change(input, { target: { value: "Work" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(mockContextsAdd).toHaveBeenCalledWith("Work"));
    await waitFor(() => expect(screen.getByText("Work")).toBeInTheDocument());
  });

  it("starts a server connection and shows 'connecting'", async () => {
    mockContextsList.mockResolvedValueOnce(contexts);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Lokal")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Server hinzufügen…"));
    const input = screen.getByPlaceholderText("Server-URL (z. B. https://notes.example.com)");
    fireEvent.change(input, { target: { value: "https://notes.example.com" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(mockServerAuthBegin).toHaveBeenCalledWith("https://notes.example.com"));
    await waitFor(() => expect(mockOpenExternal).toHaveBeenCalledWith("https://server.example.com/authorize"));
    expect(await screen.findByText("Verbinde…")).toBeInTheDocument();
  });

  it("shows an error when the server connection fails to start", async () => {
    mockContextsList.mockResolvedValueOnce(contexts);
    mockServerAuthBegin.mockRejectedValueOnce(new Error("unreachable"));
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Lokal")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Server hinzufügen…"));
    const input = screen.getByPlaceholderText("Server-URL (z. B. https://notes.example.com)");
    fireEvent.change(input, { target: { value: "https://bad.example.com" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(screen.getByText("Verbindung fehlgeschlagen")).toBeInTheDocument());
    expect(screen.queryByText("Verbinde…")).not.toBeInTheDocument();
  });

  it("renames the server context", async () => {
    mockContextsList.mockResolvedValueOnce(contexts);
    mockContextsRename.mockResolvedValueOnce([contexts[0], { ...contexts[1], label: "Team Server" }]);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());

    // Row order follows the list: [0] local ("Lokal"), [1] server ("Team").
    openActions("c-server");
    fireEvent.click(screen.getByText("Umbenennen"));
    const input = screen.getByDisplayValue("Team");
    fireEvent.change(input, { target: { value: "Team Server" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(mockContextsRename).toHaveBeenCalledWith("c-server", "Team Server"));
  });

  it("removes an inactive context, optionally deleting its file", async () => {
    mockContextsList.mockResolvedValueOnce(contexts);
    mockContextsRemove.mockResolvedValueOnce([contexts[0]]);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());

    openActions("c-server");
    fireEvent.click(screen.getByRole("button", { name: "Entfernen" }));

    fireEvent.click(screen.getByLabelText("Datenbankdatei mitlöschen"));
    const confirmButtons = screen.getAllByRole("button", { name: "Entfernen" });
    fireEvent.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() => expect(mockContextsRemove).toHaveBeenCalledWith("c-server", true));
  });

  it("shows a vault badge per context, and the change-passphrase button only where a vault exists", async () => {
    mockContextsList.mockResolvedValueOnce(contexts);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());

    // c-local has no vault: "Kein Tresor" badge, no Touch ID badge, no button.
    expect(screen.getByText("Kein Tresor")).toBeInTheDocument();
    // c-server has a vault with Touch ID enrolled: "Tresor eingerichtet" + "Touch ID" badges.
    expect(screen.getByText("Tresor eingerichtet")).toBeInTheDocument();
    expect(screen.getByText("Touch ID")).toBeInTheDocument();
    // Only the c-server menu offers the vault passphrase change; the active
    // row cannot be removed, so its menu item is inert.
    openActions("c-local");
    expect(screen.queryByRole("button", { name: "Tresor-Passwort ändern" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Entfernen" })).toBeDisabled();
    openActions("c-server");
    expect(screen.getByRole("button", { name: "Tresor-Passwort ändern" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Entfernen" })).toBeEnabled();
  });

  /** The active, workspace-bound server context — the only row the invite actions act on. */
  const serverActive = [
    { id: "c-server", label: "Team", kind: "server" as const, path: "", serverUrl: "https://s.example.com", workspaceId: "w1", active: true, vaultExists: true, vaultBiometric: false, vaultGeneration: 2, vaultRotationPending: true, role: "owner", invitesNeedingCode: 0 },
    { id: "c-local", label: "", kind: "local" as const, path: "/local.db", serverUrl: "", workspaceId: "", active: false, vaultExists: false, vaultBiometric: false, vaultGeneration: 0, vaultRotationPending: false, role: "", invitesNeedingCode: 0 },
  ];

  it("shows the workspace key generation and a pending rotation per context", async () => {
    mockContextsList.mockResolvedValueOnce(serverActive);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());

    expect(screen.getByText("Schlüssel 2")).toBeInTheDocument();
    expect(screen.getByText("Schlüsselwechsel offen")).toBeInTheDocument();
  });

  it("offers the invite actions only on the active workspace context", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault());
    mockContextsList.mockResolvedValueOnce(serverActive);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());

    // Only the active server row's menu offers the invite actions; the local row's has neither.
    openActions("c-server");
    expect(screen.getByRole("button", { name: "Tresor freigeben" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Einladungs-Code eingeben" })).toBeInTheDocument();
    openActions("c-local");
    expect(screen.queryByRole("button", { name: "Tresor freigeben" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Einladungs-Code eingeben" })).not.toBeInTheDocument();
  });

  it("offers the key change only on the active workspace context with an unlocked vault", async () => {
    mockContextsList.mockResolvedValue(serverActive);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    // Locked (the beforeEach default): the new key would have nowhere to go.
    expect(screen.queryByRole("button", { name: "Schlüssel jetzt wechseln" })).not.toBeInTheDocument();
  });

  it("rotates the key and shows one one-time code per remaining member, named or not", async () => {
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: true, biometric: false, conflict: false,
      recoveryHolder: false, rotationCode: false, recoveryMissing: false,
    });
    mockContextsList.mockResolvedValue(serverActive);
    // One member with a name (label = the name) and one without (label falls
    // back to "Mitglied {{id}}") — both branches of `VaultRotateDialog`'s
    // `onSuccess` -> `VaultCodesDialog` label mapping in one render.
    mockVaultRotate.mockResolvedValueOnce([
      { userId: 7, name: "Anna", code: "AAAAA-BBBBB" },
      { userId: 9, name: "", code: "CCCCC-DDDDD" },
    ]);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(await screen.findByRole("button", { name: "Schlüssel jetzt wechseln" }));
    // No recovery key field for a member who does not hold one.
    expect(screen.queryByPlaceholderText("Wiederherstellungs-Schlüssel")).not.toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "owner-pw" } });
    fireEvent.click(screen.getByRole("button", { name: "Schlüssel wechseln" }));

    await waitFor(() => expect(mockVaultRotate).toHaveBeenCalledWith("owner-pw", undefined));
    expect(await screen.findByText("AAAAA-BBBBB")).toBeInTheDocument();
    expect(screen.getByText("CCCCC-DDDDD")).toBeInTheDocument();
    expect(screen.getByText("Anna")).toBeInTheDocument();
    expect(screen.getByText("Mitglied 9")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Kopieren — Anna" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Fertig" }));
    await waitFor(() => expect(screen.queryByText("AAAAA-BBBBB")).not.toBeInTheDocument());
  });

  it("asks the recovery-key holder for it so the new key stays recoverable", async () => {
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: true, biometric: false, conflict: false,
      recoveryHolder: true, rotationCode: false, recoveryMissing: false,
    });
    mockContextsList.mockResolvedValue(serverActive);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(await screen.findByRole("button", { name: "Schlüssel jetzt wechseln" }));
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "owner-pw" } });
    fireEvent.change(screen.getByPlaceholderText("Wiederherstellungs-Schlüssel"), { target: { value: "AAAAA-BBBBB-CCCCC" } });
    fireEvent.click(screen.getByRole("button", { name: "Schlüssel wechseln" }));

    await waitFor(() => expect(mockVaultRotate).toHaveBeenCalledWith("owner-pw", "AAAAA-BBBBB-CCCCC"));
  });

  it("reports a wrong passphrase from the key change without minting anything", async () => {
    mockVaultStatus.mockResolvedValue({
      exists: true, unlocked: true, biometric: false, conflict: false,
      recoveryHolder: false, rotationCode: false, recoveryMissing: false,
    });
    mockContextsList.mockResolvedValue(serverActive);
    mockVaultRotate.mockRejectedValueOnce(new Error("wrong passphrase"));
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(await screen.findByRole("button", { name: "Schlüssel jetzt wechseln" }));
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "nope" } });
    fireEvent.click(screen.getByRole("button", { name: "Schlüssel wechseln" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Falsches Passwort");
    expect(screen.queryByText("AAAAA-BBBBB")).not.toBeInTheDocument();
  });

  // F2: sharing takes the DEK out of the live ring, so a locked vault gets the
  // hint instead of a button the backend would refuse.
  it("hides sharing and rotating behind the unlock hint while the vault is locked", async () => {
    mockContextsList.mockResolvedValue(serverActive); // rotation pending, vault exists
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    expect(screen.queryByRole("button", { name: "Tresor freigeben" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Schlüssel jetzt wechseln" })).not.toBeInTheDocument();
    expect(screen.getByText("Entsperre zuerst den Tresor.")).toBeInTheDocument();
    // Accepting an invitation needs no open vault, so it stays.
    expect(screen.getByRole("button", { name: "Einladungs-Code eingeben" })).toBeInTheDocument();
  });

  // F2: rotating is also pointless unless the workspace is asking for it.
  it("offers the key change only while a rotation is actually pending", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault());
    mockContextsList.mockResolvedValue(
      serverActive.map(c => ({ ...c, vaultRotationPending: false })),
    );
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    expect(screen.queryByRole("button", { name: "Schlüssel jetzt wechseln" })).not.toBeInTheDocument();
    // Sharing does not depend on a pending rotation.
    expect(screen.getByRole("button", { name: "Tresor freigeben" })).toBeInTheDocument();
  });

  it("hides the invite actions when no server context is active", async () => {
    mockContextsList.mockResolvedValueOnce(contexts); // c-local is the active one
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    expect(screen.queryByRole("button", { name: "Tresor freigeben" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Einladungs-Code eingeben" })).not.toBeInTheDocument();
  });

  it("resolves a pasted invitation link, attaches the key and shows the one-time code", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault());
    mockContextsList.mockResolvedValueOnce(serverActive);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Tresor freigeben" }));
    const input = screen.getByPlaceholderText("Einladungs-Link oder -Nummer");
    fireEvent.change(input, { target: { value: "https://s.example.com/invite/tok" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(mockInviteResolve).toHaveBeenCalledWith("https://s.example.com/invite/tok"));
    await waitFor(() => expect(mockInviteShare).toHaveBeenCalledWith(7));
    expect(await screen.findByText("ABCDE-FGHJK-MNPQR")).toBeInTheDocument();

    expect(screen.getByText(/auf einem anderen Weg weiter als den Einladungs-Link/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Fertig" }));
    await waitFor(() => expect(screen.queryByText("ABCDE-FGHJK-MNPQR")).not.toBeInTheDocument());
  });

  it("copies the one-time code to the clipboard", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    mockVaultStatus.mockResolvedValue(unlockedVault());
    mockContextsList.mockResolvedValueOnce(serverActive);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Tresor freigeben" }));
    const input = screen.getByPlaceholderText("Einladungs-Link oder -Nummer");
    fireEvent.change(input, { target: { value: "7" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText("ABCDE-FGHJK-MNPQR")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Kopieren" }));
    expect(writeText).toHaveBeenCalledWith("ABCDE-FGHJK-MNPQR");
    await waitFor(() => expect(screen.getByRole("button", { name: "Kopiert" })).toBeInTheDocument());
  });

  it("tells the invitee when their invitation link cannot be resolved", async () => {
    mockContextsList.mockResolvedValueOnce(serverActive);
    mockInviteResolve.mockRejectedValueOnce(new Error("404"));
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Einladungs-Code eingeben" }));
    fireEvent.change(screen.getByPlaceholderText("Einladungs-Link oder -Nummer"), { target: { value: "nonsense" } });
    fireEvent.change(screen.getByPlaceholderText("Einmal-Code"), { target: { value: "ABCDE" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort"), { target: { value: "member123" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort bestätigen"), { target: { value: "member123" } });
    fireEvent.click(screen.getByRole("button", { name: "Freischalten" }));

    await waitFor(() => expect(screen.getByText("Einladung nicht gefunden")).toBeInTheDocument());
    expect(mockInviteAccept).not.toHaveBeenCalled();
  });

  it("reports an invitation it cannot resolve instead of minting a code", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault());
    mockContextsList.mockResolvedValueOnce(serverActive);
    mockInviteResolve.mockRejectedValueOnce(new Error("404"));
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Tresor freigeben" }));
    const input = screen.getByPlaceholderText("Einladungs-Link oder -Nummer");
    fireEvent.change(input, { target: { value: "nonsense" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(screen.getByText("Einladung nicht gefunden")).toBeInTheDocument());
    expect(mockInviteShare).not.toHaveBeenCalled();
  });

  it("reports an attach failure in the backend's own words, not as a missing invitation", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault());
    mockContextsList.mockResolvedValueOnce(serverActive);
    mockInviteShare.mockRejectedValueOnce(new Error("vault invite HTTP 410"));
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Tresor freigeben" }));
    const input = screen.getByPlaceholderText("Einladungs-Link oder -Nummer");
    fireEvent.change(input, { target: { value: "7" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(mockInviteShare).toHaveBeenCalledWith(7));
    expect(await screen.findByText("Freigabe fehlgeschlagen: vault invite HTTP 410")).toBeInTheDocument();
    expect(screen.queryByText("Einladung nicht gefunden")).not.toBeInTheDocument();
    // No code was minted, so no code dialog.
    expect(screen.queryByText("Einmal-Code")).not.toBeInTheDocument();
  });

  it("shows how many invitations need a new code and mints them from the menu", async () => {
    mockVaultStatus.mockResolvedValue(unlockedVault());
    mockContextsList.mockResolvedValue(serverActive.map(c => c.kind === "server" ? { ...c, invitesNeedingCode: 2 } : c));
    mockInviteRecode.mockResolvedValueOnce([{ invitationId: 5, code: "AAAA-1111" }, { invitationId: 6, code: "BBBB-2222" }]);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    expect(screen.getByText("2 Einladungen brauchen einen neuen Code")).toBeInTheDocument();
    openActions("c-server");
    fireEvent.click(screen.getByRole("button", { name: "Neue Codes erzeugen" }));
    await waitFor(() => expect(mockInviteRecode).toHaveBeenCalledOnce());
    expect(await screen.findByText("Einladung 5")).toBeInTheDocument();
    expect(screen.getByText("BBBB-2222")).toBeInTheDocument();
  });

  it("redeems an invite code with a new passphrase", async () => {
    mockContextsList.mockResolvedValueOnce(serverActive);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Einladungs-Code eingeben" }));
    fireEvent.change(screen.getByPlaceholderText("Einladungs-Link oder -Nummer"), { target: { value: "https://s.example.com/invite/tok" } });
    fireEvent.change(screen.getByPlaceholderText("Einmal-Code"), { target: { value: "ABCDE-FGHJK" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort"), { target: { value: "member123" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort bestätigen"), { target: { value: "member123" } });
    fireEvent.click(screen.getByRole("button", { name: "Freischalten" }));

    await waitFor(() => expect(mockInviteAccept).toHaveBeenCalledWith(7, "ABCDE-FGHJK", "member123"));
    // The form is replaced by a confirmation the member has to acknowledge.
    expect(await screen.findByText("Tresor freigeschaltet")).toBeInTheDocument();
    expect(screen.queryByPlaceholderText("Einmal-Code")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Fertig" }));
    await waitFor(() => expect(screen.queryByText("Tresor freigeschaltet")).not.toBeInTheDocument());
  });

  it("keeps the accept dialog open and explains a rejected code", async () => {
    mockContextsList.mockResolvedValueOnce(serverActive);
    mockInviteAccept.mockRejectedValueOnce(new Error("invalid invite code"));
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Einladungs-Code eingeben" }));
    fireEvent.change(screen.getByPlaceholderText("Einladungs-Link oder -Nummer"), { target: { value: "7" } });
    fireEvent.change(screen.getByPlaceholderText("Einmal-Code"), { target: { value: "WRONG" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort"), { target: { value: "member123" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort bestätigen"), { target: { value: "member123" } });
    fireEvent.click(screen.getByRole("button", { name: "Freischalten" }));

    await waitFor(() => expect(screen.getByText("Code ungültig oder bereits eingelöst")).toBeInTheDocument());
    expect(screen.getByPlaceholderText("Einmal-Code")).toBeInTheDocument();
  });

  it("refuses to redeem when the two passphrases differ", async () => {
    mockContextsList.mockResolvedValueOnce(serverActive);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Einladungs-Code eingeben" }));
    fireEvent.change(screen.getByPlaceholderText("Einladungs-Link oder -Nummer"), { target: { value: "7" } });
    fireEvent.change(screen.getByPlaceholderText("Einmal-Code"), { target: { value: "ABCDE" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort"), { target: { value: "member123" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort bestätigen"), { target: { value: "typo" } });
    fireEvent.click(screen.getByRole("button", { name: "Freischalten" }));

    await waitFor(() => expect(screen.getByText("Passwörter stimmen nicht überein")).toBeInTheDocument());
    expect(mockInviteResolve).not.toHaveBeenCalled();
    expect(mockInviteAccept).not.toHaveBeenCalled();
  });

  it("changes a non-active context's vault passphrase without switching into it", async () => {
    mockContextsList.mockResolvedValueOnce(contexts);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Team")).toBeInTheDocument());
    openActions("c-server");

    fireEvent.click(screen.getByRole("button", { name: "Tresor-Passwort ändern" }));
    fireEvent.change(screen.getByPlaceholderText("Aktuelles Passwort"), { target: { value: "old" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort"), { target: { value: "new123" } });
    fireEvent.change(screen.getByPlaceholderText("Neues Passwort bestätigen"), { target: { value: "new123" } });
    fireEvent.click(screen.getByRole("button", { name: "Ändern" }));

    await waitFor(() => expect(mockContextsVaultChangePassphrase).toHaveBeenCalledWith("c-server", "old", "new123"));
    expect(mockVaultChangePassphrase).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByPlaceholderText("Aktuelles Passwort")).not.toBeInTheDocument());
  });
});

describe("Settings — MCP page", () => {
  it("generates a token on first visit when none exists", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    await waitFor(() => expect(onSetSetting).toHaveBeenCalledWith("mcpToken", expect.any(String)));
  });

  it("toggling enabled and auth-required calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token" }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.click(screen.getByRole("switch", { name: "Server aktiv" }));
    expect(onSetSetting).toHaveBeenCalledWith("mcpEnabled", true);
    fireEvent.click(screen.getByRole("switch", { name: "Token verpflichtend" }));
    expect(onSetSetting).toHaveBeenCalledWith("mcpAuthRequired", false);
  });

  it("changing the bind mode calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token" }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.change(screen.getByDisplayValue("Nur dieser Computer"), { target: { value: "external" } });
    expect(onSetSetting).toHaveBeenCalledWith("mcpBind", "external");
  });

  it("shows the network warning and the LAN URL when bind is external", () => {
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token", mcpBind: "external" }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    expect(screen.getByText(/im gesamten Netzwerk erreichbar/)).toBeInTheDocument();
    expect(screen.getByText("http://0.0.0.0:4357/mcp")).toBeInTheDocument();
  });

  it("changing the port calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token" }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.change(screen.getByDisplayValue("4357"), { target: { value: "8080" } });
    expect(onSetSetting).toHaveBeenCalledWith("mcpPort", 8080);
  });

  it("clamps the port to the valid range when the entered value is out of bounds", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token" }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.change(screen.getByDisplayValue("4357"), { target: { value: "99999" } });
    expect(onSetSetting).toHaveBeenCalledWith("mcpPort", 65535);
  });

  it("regenerating the token calls onSetSetting with a fresh value", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token" }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.click(screen.getByText("Neu generieren"));
    expect(onSetSetting).toHaveBeenCalledWith("mcpToken", expect.any(String));
    expect(onSetSetting).not.toHaveBeenCalledWith("mcpToken", "existing-token");
  });

  it("copies the demo config to the clipboard", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token" }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    expect(screen.getByText(/existing-token/)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Kopieren"));
    await waitFor(() => expect(writeText).toHaveBeenCalledOnce());
    expect(screen.getByText("Kopiert")).toBeInTheDocument();
  });

  it("access tab: toggling allow-write calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token" }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.click(screen.getByText("Zugriff"));
    fireEvent.click(screen.getByRole("switch", { name: "Schreiben erlauben (anlegen & ergänzen)" }));
    expect(onSetSetting).toHaveBeenCalledWith("mcpAllowWrite", true);
  });

  it("access tab: changing protected access calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token" }} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.click(screen.getByText("Zugriff"));
    fireEvent.change(screen.getByDisplayValue("Aus"), { target: { value: "read" } });
    expect(onSetSetting).toHaveBeenCalledWith("mcpProtectedAccess", "read");
  });

  it("access tab: shows the protected-access warning when it isn't off", () => {
    render(<Settings onClose={vi.fn()} settings={{ ...FULL_SETTINGS, mcpToken: "existing-token", mcpProtectedAccess: "read" }} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.click(screen.getByText("Zugriff"));
    expect(screen.getByText(/entschlüsselte geschützte Notizen/)).toBeInTheDocument();
  });
});

describe("Settings — Diagnostics page", () => {
  it("runs the checks and shows their status", async () => {
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Diagnose"));
    await waitFor(() => expect(screen.getByText("DB-Ordner schreibbar")).toBeInTheDocument());
    expect(screen.getByText("/data/notefix.db")).toBeInTheDocument();
    expect(screen.getByText("Bilder-Ordner schreibbar")).toBeInTheDocument();
    expect(screen.getByText("Fenster-Steuerung (Verschieben/Größe/Schließen)")).toBeInTheDocument();
  });

  it("offers to change the storage location when a check fails", async () => {
    mockCheckPaths.mockResolvedValueOnce({ dbWritable: false, imagesWritable: true, dbPath: "/bad/path", imagesPath: "/data/images" });
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Diagnose"));
    await waitFor(() => expect(screen.getByText("/bad/path")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Speicherort ändern…"));
    await waitFor(() => expect(mockPickFolder).toHaveBeenCalledOnce());
    expect(mockSetDbLocation).toHaveBeenCalledWith("/new");
  });

  it("re-runs the checks when 'recheck' is clicked", async () => {
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Diagnose"));
    await waitFor(() => expect(mockCheckPaths).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByText("Erneut prüfen"));
    await waitFor(() => expect(mockCheckPaths).toHaveBeenCalledTimes(2));
  });
});

describe("Settings — remaining behaviors", () => {
  it("changes the auto-lock minutes and toggles the hide/sleep auto-lock options", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    fireEvent.click(screen.getByText("Auto-Lock"));
    fireEvent.change(screen.getByDisplayValue("5"), { target: { value: "15" } });
    expect(onSetSetting).toHaveBeenCalledWith("autoLockMinutes", 15);
    fireEvent.click(screen.getByRole("switch", { name: "Beim Ausblenden sperren" }));
    expect(onSetSetting).toHaveBeenCalledWith("autoLockOnHide", false);
    fireEvent.click(screen.getByRole("switch", { name: "Bei System-Ruhezustand sperren" }));
    expect(onSetSetting).toHaveBeenCalledWith("autoLockOnSleep", false);
  });

  it("cancelling the vault-setup dialog closes it without calling the API", async () => {
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByText("Kein Tresor eingerichtet")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Tresor einrichten" }));
    expect(screen.getByPlaceholderText("Passwort")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Abbrechen"));
    expect(screen.queryByPlaceholderText("Passwort")).not.toBeInTheDocument();
    expect(mockVaultSetup).not.toHaveBeenCalled();
  });

  it("changing trash retention, autosave delay and start view calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={onSetSetting} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.change(screen.getByDisplayValue("30"), { target: { value: "60" } });
    expect(onSetSetting).toHaveBeenCalledWith("trashRetentionDays", 60);
    fireEvent.change(screen.getByDisplayValue("400"), { target: { value: "800" } });
    expect(onSetSetting).toHaveBeenCalledWith("autosaveDelay", 800);
    fireEvent.change(screen.getByDisplayValue("Zuletzt geöffnete Notiz"), { target: { value: "dashboard" } });
    expect(onSetSetting).toHaveBeenCalledWith("startView", "dashboard");
  });

  it("disabling start-on-boot calls autostart.disable", async () => {
    mockIsEnabled.mockResolvedValueOnce(true);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    const toggle = await screen.findByRole("switch", { name: "Bei Anmeldung starten" });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    fireEvent.click(toggle);
    await waitFor(() => expect(mockDisable).toHaveBeenCalledOnce());
  });

  it("shows the 'switched to existing db' message when the location change switches instead of moves", async () => {
    mockSetDbLocation.mockResolvedValueOnce({ mode: "switched", path: "/existing/notefix.db" });
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Ändern…"));
    await waitFor(() => expect(screen.getByText(/Gewechselt zur vorhandenen DB/)).toBeInTheDocument());
  });

  it("does nothing when the folder picker is cancelled", async () => {
    mockPickFolder.mockResolvedValueOnce(null);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Ändern…"));
    await waitFor(() => expect(mockPickFolder).toHaveBeenCalledOnce());
    expect(mockSetDbLocation).not.toHaveBeenCalled();
  });

  it("refreshes the context list when a server auth completes in the background", async () => {
    const contexts = [{ id: "c-local", label: "", kind: "local" as const, path: "/local.db", serverUrl: "", workspaceId: "", active: true }];
    mockContextsList.mockResolvedValueOnce(contexts).mockResolvedValueOnce(contexts);
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Kontexte"));
    await waitFor(() => expect(screen.getByText("Lokal")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Server hinzufügen…"));
    fireEvent.change(screen.getByPlaceholderText("Server-URL (z. B. https://notes.example.com)"), { target: { value: "https://notes.example.com" } });
    fireEvent.keyDown(screen.getByPlaceholderText("Server-URL (z. B. https://notes.example.com)"), { key: "Enter" });
    expect(await screen.findByText("Verbinde…")).toBeInTheDocument();

    // Simulate the `context-changed` event that fires once the notefix:// auth
    // callback lands. Several components subscribe (the page itself, and
    // `useVault`), and the real event reaches all of them.
    for (const [cb] of mockOnContextChanged.mock.calls) cb();

    await waitFor(() => expect(screen.queryByText("Verbinde…")).not.toBeInTheDocument());
    expect(mockContextsList).toHaveBeenCalledTimes(2);
  });
});

describe("SecurityPage — passphrase wording", () => {
  it("explains that lock, unlock and encryption share the one vault passphrase", async () => {
    mockVaultStatus.mockResolvedValue({ exists: true, unlocked: true, biometric: false, conflict: false, recoveryHolder: true, rotationCode: false, recoveryMissing: false, sealOutdated: false });
    render(<Settings onClose={vi.fn()} settings={FULL_SETTINGS} onSetSetting={vi.fn()} onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Sicherheit"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Passwort ändern" })).toBeInTheDocument());
    expect(screen.getByText(/Sperren und Entsperren nutzen dasselbe Passwort/)).toBeInTheDocument();
  });
});
