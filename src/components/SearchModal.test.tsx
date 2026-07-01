import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import SearchModal from './SearchModal';

const { search, searchAll } = vi.hoisted(() => ({
  search: vi.fn().mockResolvedValue([
    { note: { id: 'n1', preview: 'Apfel Notiz', tasksDone: 0, tasksTotal: 0, updatedAt: 0, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null }, snippet: '…Apfel Notiz…' },
  ]),
  searchAll: vi.fn().mockResolvedValue([
    { contextId: 'c1', contextLabel: 'Privat', kind: 'local', note: { id: 'g1', preview: 'Apfel global', tasksDone: 0, tasksTotal: 0, updatedAt: 0, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null }, snippet: '…Apfel global…' },
  ]),
}));

vi.mock('../api', () => ({
  api: { notes: { search, searchAll } },
}));

describe('SearchModal', () => {
  it('shows the results the context search returns', async () => {
    render(<SearchModal scope="context" onScope={() => {}} onClose={() => {}} onOpenNote={() => {}} />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'apfel' } });
    await screen.findByRole('button', { name: /Apfel Notiz/i });
    expect(search).toHaveBeenCalledWith('apfel');
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

  it('searches all contexts and opens with the context id when scope is global', async () => {
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
