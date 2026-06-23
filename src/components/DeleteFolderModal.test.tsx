import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import DeleteFolderModal from "./DeleteFolderModal";

describe("DeleteFolderModal", () => {
  it("wires the three actions", () => {
    const onReparent = vi.fn(), onRecursive = vi.fn(), onCancel = vi.fn();
    render(<DeleteFolderModal folderName="A" noteCount={2} subfolderCount={1} onReparent={onReparent} onRecursive={onRecursive} onCancel={onCancel} />);
    fireEvent.click(screen.getByText(/Inhalt in Elternordner/));
    fireEvent.click(screen.getByText("Alles löschen"));
    fireEvent.click(screen.getByText("Abbrechen"));
    expect(onReparent).toHaveBeenCalledOnce();
    expect(onRecursive).toHaveBeenCalledOnce();
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
