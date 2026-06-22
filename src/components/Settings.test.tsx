import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("../api", () => ({
  api: { getAppInfo: vi.fn(() => Promise.resolve({ name: "Notefix", version: "0.1.0", description: "x" })) },
}));

import Settings from "./Settings";

beforeEach(() => vi.clearAllMocks());

describe("Settings — Darstellung", () => {
  it("switching to sections calls onSetSetting", async () => {
    const onSetSetting = vi.fn();
    render(<Settings onClose={vi.fn()} settings={{ pinnedDisplayMode: "flat" }} onSetSetting={onSetSetting} />);
    fireEvent.click(screen.getByText("Darstellung"));
    fireEvent.click(screen.getByText(/Sektionen/));
    expect(onSetSetting).toHaveBeenCalledWith("pinnedDisplayMode", "sections");
  });

  it("shows the About page by default", async () => {
    render(<Settings onClose={vi.fn()} settings={{ pinnedDisplayMode: "flat" }} onSetSetting={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("Notefix")).toBeInTheDocument());
  });
});
