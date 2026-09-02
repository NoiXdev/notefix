import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import SettingsSection from "./SettingsSection";

describe("SettingsSection", () => {
  it("renders the title and the children", () => {
    render(
      <SettingsSection title="Status">
        <p>Entsperrt</p>
      </SettingsSection>,
    );
    expect(screen.getByText("Status")).toBeInTheDocument();
    expect(screen.getByText("Entsperrt")).toBeInTheDocument();
  });

  it("renders multiple children in a single section", () => {
    render(
      <SettingsSection title="Optionen">
        <span>Row one</span>
        <span>Row two</span>
      </SettingsSection>,
    );
    expect(screen.getByText("Row one")).toBeInTheDocument();
    expect(screen.getByText("Row two")).toBeInTheDocument();
  });
});
