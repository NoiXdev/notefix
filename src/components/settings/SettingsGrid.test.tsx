import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import SettingsGrid from "./SettingsGrid";

describe("SettingsGrid", () => {
  it("renders its children in a responsive grid container", () => {
    const { container } = render(
      <SettingsGrid>
        <section>First</section>
        <section>Second</section>
      </SettingsGrid>,
    );
    expect(screen.getByText("First")).toBeInTheDocument();
    expect(screen.getByText("Second")).toBeInTheDocument();
    expect(container.firstChild).toHaveClass("grid", "grid-cols-1", "md:grid-cols-2");
  });
});
