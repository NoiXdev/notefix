import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import SettingRow from "./SettingRow";

describe("SettingRow", () => {
  it("renders the label and the control", () => {
    render(
      <SettingRow label="Sperrbereich">
        <button>Ändern</button>
      </SettingRow>,
    );
    expect(screen.getByText("Sperrbereich")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ändern" })).toBeInTheDocument();
  });

  it("renders the hint under the label when provided, and omits it otherwise", () => {
    const { rerender } = render(
      <SettingRow label="Speicherort" hint="/data/notefix.db">
        <button>Ändern…</button>
      </SettingRow>,
    );
    expect(screen.getByText("/data/notefix.db")).toBeInTheDocument();

    rerender(
      <SettingRow label="Speicherort">
        <button>Ändern…</button>
      </SettingRow>,
    );
    expect(screen.queryByText("/data/notefix.db")).not.toBeInTheDocument();
  });

  it("uses a column layout (label above control) when stack is set, and a row layout otherwise", () => {
    const { container, rerender } = render(
      <SettingRow label="Theme">
        <button>Butter</button>
      </SettingRow>,
    );
    expect(container.firstChild).toHaveClass("items-center", "justify-between");

    rerender(
      <SettingRow label="Theme" stack>
        <button>Butter</button>
      </SettingRow>,
    );
    expect(container.firstChild).toHaveClass("flex-col");
    expect(container.firstChild).not.toHaveClass("items-center");
  });
});
