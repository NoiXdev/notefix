import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';

vi.mock('../api', () => ({
  api: {
    notes: {
      revisions: vi.fn(() => Promise.resolve([{ id: 1, noteId: 'n', createdAt: 1000 }])),
      revisionContent: vi.fn(() => Promise.resolve('<p>old</p>')),
    },
  },
}));

import HistoryModal from './HistoryModal';

describe('HistoryModal', () => {
  it('lists revisions, previews one, and restores it', async () => {
    const onRestore = vi.fn();
    render(<HistoryModal noteId="n" onRestore={onRestore} onClose={vi.fn()} />);
    fireEvent.click(await screen.findByLabelText('Version 1'));
    await waitFor(() => expect(screen.getByText('old')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Wiederherstellen'));
    expect(onRestore).toHaveBeenCalledWith('<p>old</p>');
  });
});
