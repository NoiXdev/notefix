import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

const {
  mockIsEnabled,
  mockEnable,
  mockDisable,
  mockExportSelected,
  mockGetDbPath,
  mockSetDbLocation,
  mockRelaunch,
  mockPickFolder,
} = vi.hoisted(() => ({
  mockIsEnabled: vi.fn(() => Promise.resolve(false)),
  mockEnable: vi.fn(() => Promise.resolve()),
  mockDisable: vi.fn(() => Promise.resolve()),
  mockExportSelected: vi.fn(),
  mockGetDbPath: vi.fn(() => Promise.resolve("/data/notefix.db")),
  mockSetDbLocation: vi.fn(() => Promise.resolve({ mode: "moved", path: "/new/notefix.db" })),
  mockRelaunch: vi.fn(),
  mockPickFolder: vi.fn(() => Promise.resolve("/new")),
}));

vi.mock("../api", () => ({
  api: {
    getAppInfo: vi.fn(() => Promise.resolve({ name: "Notefix", version: "0.1.0", description: "x" })),
    autostart: { isEnabled: mockIsEnabled, enable: mockEnable, disable: mockDisable },
    stats: vi.fn(() => Promise.resolve({ notes: 3, archived: 1, characters: 42, words: 8 })),
    getDbPath: mockGetDbPath,
    setDbLocation: mockSetDbLocation,
    relaunch: mockRelaunch,
    pickFolder: mockPickFolder,
  },
}));
vi.mock("../export", () => ({ exportSelected: mockExportSelected }));

vi.mock('react-select', () => ({
  default: ({ options, value, onChange }: { options: { value: string; label: string }[]; value: { value: string } | null; onChange: (o: { value: string }) => void }) => (
    <select aria-label="select" value={value?.value ?? ''} onChange={e => onChange(options.find(o => o.value === e.target.value)!)}>
      {options.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
    </select>
  ),
}));

import Settings from "./Settings";

beforeEach(() => vi.clearAllMocks());

describe("Settings — Darstellung", () => {
  it("shows the About page by default", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder", folderColorStyle: "icon" }} onSetSetting={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("Notefix")).toBeInTheDocument());
  });

  it("shows the logo on the About page", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder", folderColorStyle: "icon" }} onSetSetting={vi.fn()} />);
    expect(await screen.findByAltText("Notefix")).toBeInTheDocument();
  });
});

describe("Settings — System", () => {
  it("toggling start-minimized calls onSetSetting", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/Minimiert starten/));
    expect(onSetSetting).toHaveBeenCalledWith("startMinimized", true);
  });

  it("enabling start-on-boot calls autostart.enable", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/Bei Anmeldung starten/));
    expect(mockEnable).toHaveBeenCalledOnce();
  });

  it("'export all' calls exportSelected with empty ids", () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Alle als JSON exportieren"));
    expect(mockExportSelected).toHaveBeenCalledWith([], "notefix-export.json");
  });
});

describe("Settings — date format & stats", () => {
  it("selecting a date format calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.change(screen.getByDisplayValue("Auto (relativ)"), { target: { value: "iso" } });
    expect(onSetSetting).toHaveBeenCalledWith("dateFormat", "iso");
  });

  it("stats page shows the counts", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} />);
    fireEvent.click(screen.getByText("Statistik"));
    await waitFor(() => expect(screen.getByText("42")).toBeInTheDocument());
  });
});

describe("Settings — Speicherort", () => {
  it("shows the db path and changes location then offers restart", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} />);
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
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.change(screen.getByDisplayValue("Nur Icon einfärben"), { target: { value: "row" } });
    expect(onSetSetting).toHaveBeenCalledWith("folderColorStyle", "row");
  });
});

describe("Settings — pinnedScope", () => {
  it("selecting global calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.change(screen.getByDisplayValue("Gepinnt zuerst je Ordner"), { target: { value: "global" } });
    expect(onSetSetting).toHaveBeenCalledWith("pinnedScope", "global");
  });
});

describe("Settings — editor & history", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }] };
  it("changing the revision limit calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.change(screen.getByDisplayValue("50"), { target: { value: "10" } });
    expect(onSetSetting).toHaveBeenCalledWith("revisionLimit", 10);
  });
});

describe("Settings — tree view", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true };
  it("toggling compact view calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByLabelText(/Kompakte Ansicht/));
    expect(onSetSetting).toHaveBeenCalledWith("compactTree", true);
  });
});

describe("Settings — trash", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const };
  it("toggling trash calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/Papierkorb verwenden/));
    expect(onSetSetting).toHaveBeenCalledWith("trashEnabled", false);
  });
});

describe("Settings — shortcuts page", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const };
  it("lists the new-note shortcut", () => {
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={vi.fn()} />);
    fireEvent.click(screen.getByText("Tastatur"));
    expect(screen.getByText("Neue Notiz")).toBeInTheDocument();
  });
});

describe("Settings — close behavior", () => {
  const full = { startMinimized: false, dateFormat: "auto" as const, pinnedScope: "perFolder" as const, folderColorStyle: "icon" as const, revisionLimit: 50, autosaveDelay: 400, startView: "lastNote" as const, dashboardLayout: [{ key: "recent", x: 0, y: 0, w: 6, h: 4 }], compactTree: false, treeProgress: true, trashEnabled: true, trashRetentionDays: 30, closeAction: "ask" as const };
  it("changing close behavior calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={full} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.change(screen.getByDisplayValue("Fragen"), { target: { value: "quit" } });
    expect(onSetSetting).toHaveBeenCalledWith("closeAction", "quit");
  });
});
