import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import SearchModal from './SearchModal';

const { load, loadAll } = vi.hoisted(() => ({
  load: vi.fn().mockResolvedValue([
    { id: 'n1', content: '<p>Apfel Notiz</p>', updatedAt: 5, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null },
    { id: 'n2', content: '<p>Birne Notiz</p>', updatedAt: 9, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null },
  ]),
  loadAll: vi.fn().mockResolvedValue([
    { contextId: 'c1', contextLabel: 'Privat', kind: 'local', note: { id: 'g1', content: '<p>Apfel global</p>', updatedAt: 5, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null } },
  ]),
}));

vi.mock('../api', () => ({
  api: { notes: { load, loadAll } },
}));

describe('SearchModal', () => {
  it('filters context notes by query and hides non-matches', async () => {
    render(<SearchModal scope="context" onScope={() => {}} onClose={() => {}} onOpenNote={() => {}} />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'apfel' } });
    await screen.findByRole('button', { name: /Apfel Notiz/i });
    expect(screen.queryByRole('button', { name: /Birne/i })).not.toBeInTheDocument();
  });

  it('opens the selected note on Enter with its id', async () => {
    const onOpenNote = vi.fn();
    render(<SearchModal scope="context" onScope={() => {}} onClose={() => {}} onOpenNote={onOpenNote} />);
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'apfel' } });
    await screen.findByRole('button', { name: /Apfel Notiz/i });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(onOpenNote).toHaveBeenCalledWith('n1', undefined));
  });

  it('loads global notes with their context id and label when scope is global', async () => {
    const onOpenNote = vi.fn();
    render(<SearchModal scope="global" onScope={() => {}} onClose={() => {}} onOpenNote={onOpenNote} />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'apfel' } });
    fireEvent.click(await screen.findByRole('button', { name: /Apfel global/i }));
    expect(onOpenNote).toHaveBeenCalledWith('g1', 'c1');
  });

  it('reports the requested scope when a scope button is clicked', () => {
    const onScope = vi.fn();
    render(<SearchModal scope="context" onScope={onScope} onClose={() => {}} onOpenNote={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: 'Global' }));
    expect(onScope).toHaveBeenCalledWith('global');
  });
});
