import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import VaultConflictDialog from "./VaultConflictDialog";
import "../i18n";

describe("VaultConflictDialog", () => {
  it("submits both secrets and the merge mode, then shows the count", async () => {
    const resolve = vi.fn().mockResolvedValue({ changed: 3, skipped: 0 });
    render(<VaultConflictDialog resolve={resolve} onClose={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText("Passwort des Arbeitsbereichs"), { target: { value: "ws" } });
    fireEvent.change(screen.getByPlaceholderText("Lokales Passwort"), { target: { value: "local" } });
    fireEvent.click(screen.getByRole("button", { name: "Konflikt lösen" }));
    await waitFor(() => expect(resolve).toHaveBeenCalledWith("ws", { kind: "passphrase", value: "local" }, "merge"));
    expect(await screen.findByText("3 Notizen übernommen")).toBeInTheDocument();
  });

  it("switches the local secret to the recovery key and the mode to unprotect", async () => {
    const resolve = vi.fn().mockResolvedValue({ changed: 2, skipped: 1 });
    render(<VaultConflictDialog resolve={resolve} onClose={vi.fn()} />);
    fireEvent.click(screen.getByText("Wiederherstellungs-Schlüssel verwenden"));
    fireEvent.change(screen.getByPlaceholderText("Passwort des Arbeitsbereichs"), { target: { value: "ws" } });
    fireEvent.change(screen.getByPlaceholderText("Lokaler Wiederherstellungs-Schlüssel"), { target: { value: "KEY" } });
    fireEvent.click(screen.getByLabelText("Notizen entschützen"));
    fireEvent.click(screen.getByRole("button", { name: "Konflikt lösen" }));
    await waitFor(() => expect(resolve).toHaveBeenCalledWith("ws", { kind: "recovery", value: "KEY" }, "unprotect"));
    expect(await screen.findByText("2 Notizen entschützt")).toBeInTheDocument();
    expect(screen.getByText(/1 Notizen konnten mit keinem Schlüssel/)).toBeInTheDocument();
  });

  it("maps the backend errors", async () => {
    const resolve = vi.fn().mockRejectedValueOnce(new Error("wrong passphrase")).mockRejectedValueOnce(new Error("vault: local record does not open"));
    render(<VaultConflictDialog resolve={resolve} onClose={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText("Passwort des Arbeitsbereichs"), { target: { value: "a" } });
    fireEvent.change(screen.getByPlaceholderText("Lokales Passwort"), { target: { value: "b" } });
    fireEvent.click(screen.getByRole("button", { name: "Konflikt lösen" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Falsches Passwort");
    fireEvent.click(screen.getByRole("button", { name: "Konflikt lösen" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Das lokale Passwort oder der Wiederherstellungs-Schlüssel ist falsch."));
  });

  it("shows the warning that members will be able to read the notes", () => {
    render(<VaultConflictDialog resolve={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByText(/für alle Mitglieder des Arbeitsbereichs lesbar/)).toBeInTheDocument();
  });
});
