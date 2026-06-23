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

describe("ContextMenu — swatches", () => {
  it("renders swatches and a clear button; picking calls onPick", () => {
    const onPick = vi.fn();
    render(
      <ContextMenu
        x={10}
        y={10}
        items={[]}
        swatches={{ colors: ["#ef4444", "#22c55e"], current: "", onPick }}
        onClose={vi.fn()}
      />
    );
    fireEvent.click(screen.getByLabelText("Farbe #ef4444"));
    expect(onPick).toHaveBeenCalledWith("#ef4444");
    fireEvent.click(screen.getByLabelText("Keine Farbe"));
    expect(onPick).toHaveBeenCalledWith("");
  });
});

describe("ContextMenu — submenu", () => {
  it("renders submenu entries and clicking one fires its handler + onClose", () => {
    const onPick = vi.fn();
    const onClose = vi.fn();
    render(
      <ContextMenu
        x={10} y={10} onClose={onClose}
        items={[{ label: "Verschieben nach", submenu: [{ label: "Ordner A", onClick: onPick }] }]}
      />
    );
    expect(screen.getByText("Verschieben nach")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Ordner A"));
    expect(onPick).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });
});
