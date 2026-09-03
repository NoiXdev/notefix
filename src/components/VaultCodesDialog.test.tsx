import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import VaultCodesDialog from './VaultCodesDialog';

const entries = [
  { id: 'member-2', label: 'Mitglied 2', code: 'AAAAA-BBBBB' },
  { id: 'member-3', label: 'Mitglied 3', code: 'CCCCC-DDDDD' },
];

describe('VaultCodesDialog', () => {
  it('lists one code per entry', () => {
    render(<VaultCodesDialog title="Einmal-Codes" hint="H" entries={entries} onClose={vi.fn()} />);
    expect(screen.getByText('Einmal-Codes')).toBeInTheDocument();
    expect(screen.getByText('Mitglied 2')).toBeInTheDocument();
    expect(screen.getByText('AAAAA-BBBBB')).toBeInTheDocument();
    expect(screen.getByText('Mitglied 3')).toBeInTheDocument();
    expect(screen.getByText('CCCCC-DDDDD')).toBeInTheDocument();
  });

  it('labels each copy button with the entry it belongs to', () => {
    render(<VaultCodesDialog title="T" hint="H" entries={entries} onClose={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Kopieren — Mitglied 2' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Kopieren — Mitglied 3' })).toBeInTheDocument();
  });

  it('copies one entry’s code and shows the confirmation only on that row', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(<VaultCodesDialog title="T" hint="H" entries={entries} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Kopieren — Mitglied 2' }));
    expect(writeText).toHaveBeenCalledWith('AAAAA-BBBBB');
    await waitFor(() => expect(screen.getByText('Kopiert')).toBeInTheDocument());
    // The other row keeps its copy button.
    expect(screen.getByRole('button', { name: 'Kopieren — Mitglied 3' })).toBeInTheDocument();
  });

  it('reports a refused clipboard write instead of claiming success', async () => {
    const writeText = vi.fn(() => Promise.reject(new Error('denied')));
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(<VaultCodesDialog title="T" hint="H" entries={entries} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Kopieren — Mitglied 2' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Kopieren fehlgeschlagen');
    expect(screen.queryByText('Kopiert')).not.toBeInTheDocument();
  });

  it('says the empty state when there is nothing left to show', () => {
    render(<VaultCodesDialog title="T" hint="H" entries={[]} onClose={vi.fn()} />);
    expect(screen.getByText('Schlüssel gewechselt')).toBeInTheDocument();
  });

  it('labels entries with the given label and names the copy button after it', () => {
    render(<VaultCodesDialog title="T" hint="H" entries={[{ id: 'inv-5', label: 'Einladung 5', code: 'AAAA-BBBB' }]} onClose={vi.fn()} />);
    expect(screen.getByText('Einladung 5')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Kopieren — Einladung 5' })).toBeInTheDocument();
  });

  it('closes on the acknowledge button', () => {
    const onClose = vi.fn();
    render(<VaultCodesDialog title="T" hint="H" entries={entries} onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: 'Fertig' }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
