import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import SettingsTabs from "./SettingsTabs";

const tabs = [
  { id: "vault", label: "Tresor" },
  { id: "autoLock", label: "Auto-Lock" },
];

describe("SettingsTabs", () => {
  it("renders every tab's label", () => {
    render(<SettingsTabs tabs={tabs} active="vault" onChange={vi.fn()} />);
    expect(screen.getByText("Tresor")).toBeInTheDocument();
    expect(screen.getByText("Auto-Lock")).toBeInTheDocument();
  });

  it("styles the active tab differently from inactive ones", () => {
    render(<SettingsTabs tabs={tabs} active="vault" onChange={vi.fn()} />);
    const active = screen.getByText("Tresor");
    const inactive = screen.getByText("Auto-Lock");
    expect(active.style.borderColor).toBe("var(--accent-strong)");
    expect(inactive.style.borderColor).toBe("transparent");
    expect(active.style.color).not.toBe(inactive.style.color);
  });

  it("calls onChange with the clicked tab's id", () => {
    const onChange = vi.fn();
    render(<SettingsTabs tabs={tabs} active="vault" onChange={onChange} />);
    fireEvent.click(screen.getByText("Auto-Lock"));
    expect(onChange).toHaveBeenCalledWith("autoLock");
    expect(onChange).not.toHaveBeenCalledWith("vault");
  });
});
