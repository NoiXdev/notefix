import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import SystemCheckModal from './SystemCheckModal';

describe('SystemCheckModal', () => {
  it('lists problems and opens settings', () => {
    const onOpenSettings = vi.fn(), onClose = vi.fn();
    render(<SystemCheckModal problems={[{ key: 'db', label: 'DB-Ordner schreibbar', status: 'error', detail: '/db' }]} onOpenSettings={onOpenSettings} onClose={onClose} />);
    expect(screen.getByText('DB-Ordner schreibbar')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Einstellungen öffnen'));
    expect(onOpenSettings).toHaveBeenCalled();
  });
});
