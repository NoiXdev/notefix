import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import type { ReleaseInfo } from "../api";

const { mockOpenExternal } = vi.hoisted(() => ({
  mockOpenExternal: vi.fn(),
}));

vi.mock("../api", () => ({
  api: { openExternal: mockOpenExternal },
}));

import WhatsNew from "./WhatsNew";

const releases: ReleaseInfo[] = [
  {
    tagName: "v0.6.0",
    name: "v0.6.0 — Apps page",
    body: "### Added\n- Apps page\n\n### Fixed\n- Sync race",
    publishedAt: "2026-08-20T10:00:00Z",
    prerelease: false,
  },
  {
    tagName: "v0.5.1",
    name: "",
    body: "- Minor fixes",
    publishedAt: "2026-07-01T08:30:00Z",
    prerelease: true,
  },
];

describe("WhatsNew", () => {
  it("renders each release's name (or tag as fallback) and its body", () => {
    render(<WhatsNew releases={releases} onClose={vi.fn()} />);
    expect(screen.getByText("v0.6.0 — Apps page")).toBeInTheDocument();
    expect(screen.getByText("v0.5.1")).toBeInTheDocument(); // falls back to tagName when name is empty
    expect(screen.getByText("Added")).toBeInTheDocument();
    expect(screen.getByText("Minor fixes")).toBeInTheDocument();
  });

  it("shows the empty state when there are no releases", () => {
    render(<WhatsNew releases={[]} onClose={vi.fn()} />);
    expect(screen.getByText("Noch keine Einträge im Änderungsprotokoll.")).toBeInTheDocument();
  });

  it("calls onClose when the close button is clicked", () => {
    const onClose = vi.fn();
    render(<WhatsNew releases={releases} onClose={onClose} />);
    fireEvent.click(screen.getAllByText("Schließen")[0]);
    expect(onClose).toHaveBeenCalled();
  });

  it("opens the release on GitHub via the opener api, not a raw navigation", () => {
    render(<WhatsNew releases={releases} onClose={vi.fn()} />);
    fireEvent.click(screen.getAllByText("Auf GitHub ansehen")[0]);
    expect(mockOpenExternal).toHaveBeenCalledWith("https://github.com/NoiXdev/notefix/releases/tag/v0.6.0");
  });

  it("does not render a raw <script> tag from a malicious release body", () => {
    const malicious: ReleaseInfo[] = [
      { tagName: "v0.6.1", name: "evil", body: "<script>window.hacked = true</script>", publishedAt: "2026-08-21T00:00:00Z", prerelease: false },
    ];
    const { container } = render(<WhatsNew releases={malicious} onClose={vi.fn()} />);
    expect(container.querySelector("script")).toBeNull();
  });
});
