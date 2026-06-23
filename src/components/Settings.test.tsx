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

import Settings from "./Settings";

beforeEach(() => vi.clearAllMocks());

describe("Settings — Darstellung", () => {
  it("shows the About page by default", async () => {
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("Notefix")).toBeInTheDocument());
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
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText("JJJJ-MM-TT"));
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

describe("Settings — pinnedScope", () => {
  it("selecting global calls onSetSetting", () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ startMinimized: false, dateFormat: "auto", pinnedScope: "perFolder" }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText(/Globale/));
    expect(onSetSetting).toHaveBeenCalledWith("pinnedScope", "global");
  });
});
