import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

const { mockIsEnabled, mockEnable, mockDisable } = vi.hoisted(() => ({
  mockIsEnabled: vi.fn(() => Promise.resolve(false)),
  mockEnable: vi.fn(() => Promise.resolve()),
  mockDisable: vi.fn(() => Promise.resolve()),
}));

vi.mock("../api", () => ({
  api: {
    getAppInfo: vi.fn(() => Promise.resolve({ name: "Notefix", version: "0.1.0", description: "x" })),
    autostart: { isEnabled: mockIsEnabled, enable: mockEnable, disable: mockDisable },
  },
}));

import Settings from "./Settings";

beforeEach(() => vi.clearAllMocks());

describe("Settings — Darstellung", () => {
  it("switching to sections calls onSetSetting", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ pinnedDisplayMode: "flat", startMinimized: false }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText(/Sektionen/));
    expect(onSetSetting).toHaveBeenCalledWith("pinnedDisplayMode", "sections");
  });

  it("shows the About page by default", async () => {
    render(<Settings onClose={vi.fn()} settings={{ pinnedDisplayMode: "flat", startMinimized: false }} onSetSetting={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("Notefix")).toBeInTheDocument());
  });
});

describe("Settings — System", () => {
  it("toggling start-minimized calls onSetSetting", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ pinnedDisplayMode: "flat", startMinimized: false }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/Minimiert starten/));
    expect(onSetSetting).toHaveBeenCalledWith("startMinimized", true);
  });

  it("enabling start-on-boot calls autostart.enable", async () => {
    render(<Settings onClose={vi.fn()} settings={{ pinnedDisplayMode: "flat", startMinimized: false }} onSetSetting={vi.fn()} />);
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByLabelText(/Bei Anmeldung starten/));
    expect(mockEnable).toHaveBeenCalledOnce();
  });
});
