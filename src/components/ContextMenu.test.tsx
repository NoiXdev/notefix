import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import ContextMenu from "./ContextMenu";

describe("ContextMenu", () => {
  it("renders its items", () => {
    render(<ContextMenu x={10} y={10} items={[{ label: "Anpinnen", onClick: vi.fn() }]} onClose={vi.fn()} />);
    expect(screen.getByText("Anpinnen")).toBeInTheDocument();
  });

  it("calls the item handler and onClose when clicked", () => {
    const onClick = vi.fn();
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} items={[{ label: "Anpinnen", onClick }]} onClose={onClose} />);
    fireEvent.click(screen.getByText("Anpinnen"));
    expect(onClick).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("calls onClose on Escape", () => {
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} items={[{ label: "x", onClick: vi.fn() }]} onClose={onClose} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });
});
