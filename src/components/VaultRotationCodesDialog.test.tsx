import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import VaultRotationCodesDialog from './VaultRotationCodesDialog';

const codes = [
  { userId: 2, code: 'AAAAA-BBBBB' },
  { userId: 3, code: 'CCCCC-DDDDD' },
];

describe('VaultRotationCodesDialog', () => {
  it('lists one code per member', () => {
    render(<VaultRotationCodesDialog codes={codes} onClose={vi.fn()} />);
    expect(screen.getByText('Einmal-Codes')).toBeInTheDocument();
    expect(screen.getByText('Mitglied 2')).toBeInTheDocument();
    expect(screen.getByText('AAAAA-BBBBB')).toBeInTheDocument();
    expect(screen.getByText('Mitglied 3')).toBeInTheDocument();
    expect(screen.getByText('CCCCC-DDDDD')).toBeInTheDocument();
  });

  it('labels each copy button with the member it belongs to', () => {
    render(<VaultRotationCodesDialog codes={codes} onClose={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Kopieren — Mitglied 2' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Kopieren — Mitglied 3' })).toBeInTheDocument();
  });

  it('copies one member’s code and shows the confirmation only on that row', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(<VaultRotationCodesDialog codes={codes} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Kopieren — Mitglied 2' }));
    expect(writeText).toHaveBeenCalledWith('AAAAA-BBBBB');
    await waitFor(() => expect(screen.getByText('Kopiert')).toBeInTheDocument());
    // The other row keeps its copy button.
    expect(screen.getByRole('button', { name: 'Kopieren — Mitglied 3' })).toBeInTheDocument();
  });

  it('reports a refused clipboard write instead of claiming success', async () => {
    const writeText = vi.fn(() => Promise.reject(new Error('denied')));
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(<VaultRotationCodesDialog codes={codes} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Kopieren — Mitglied 2' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Kopieren fehlgeschlagen');
    expect(screen.queryByText('Kopiert')).not.toBeInTheDocument();
  });

  it('says the key changed when a lone member rotated for themselves', () => {
    render(<VaultRotationCodesDialog codes={[]} onClose={vi.fn()} />);
    expect(screen.getByText('Schlüssel gewechselt')).toBeInTheDocument();
  });

  it('closes on the acknowledge button', () => {
    const onClose = vi.fn();
    render(<VaultRotationCodesDialog codes={codes} onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: 'Fertig' }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
