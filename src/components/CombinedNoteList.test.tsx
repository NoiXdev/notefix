import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import CombinedNoteList from './CombinedNoteList';

const { loadAll, contextsList, contextSwitch } = vi.hoisted(() => ({
  loadAll: vi.fn().mockResolvedValue([
    { contextId: 'c1', contextLabel: 'Privat', kind: 'local', note: { id: 'n1', preview: 'Hallo', tasksDone: 0, tasksTotal: 0, updatedAt: 5, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null } },
    { contextId: 'c2', contextLabel: 'srv', kind: 'server', note: { id: 'n2', preview: 'Welt', tasksDone: 0, tasksTotal: 0, updatedAt: 9, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null } },
  ]),
  contextsList: vi.fn().mockResolvedValue([
    { id: 'c1', label: 'Privat', kind: 'local', path: '', serverUrl: '', workspaceId: '', active: true },
    { id: 'c2', label: 'srv', kind: 'server', path: '', serverUrl: 'https://s', workspaceId: 'w', active: false },
  ]),
  contextSwitch: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../api', () => ({
  api: {
    notes: { loadAll },
    contexts: {
      list: contextsList,
      switch: contextSwitch,
      syncStatus: vi.fn().mockResolvedValue({ state: 'local', lastSyncedAt: 0, pending: 0 }),
    },
    onNotesChanged: () => () => {},
    onContextChanged: () => () => {},
    onSyncStatus: () => () => {},
  },
}));

describe('CombinedNoteList', () => {
  it('renders notes from all contexts with their badges', async () => {
    render(<CombinedNoteList selectedId={null} activeContextId="c1" onSelectNote={() => {}} onCreate={() => {}} onOpenSettings={() => {}} onOpenContexts={() => {}} dateFormat="auto" />);
    expect(await screen.findByText('Hallo')).toBeInTheDocument();
    expect(await screen.findByText('Welt')).toBeInTheDocument();
    expect(screen.getAllByText('srv').length).toBeGreaterThan(0);
  });

  it('calls onSelectNote with the note id and its context id', async () => {
    const onSelectNote = vi.fn();
    render(<CombinedNoteList selectedId={null} activeContextId="c1" onSelectNote={onSelectNote} onCreate={() => {}} onOpenSettings={() => {}} onOpenContexts={() => {}} dateFormat="auto" />);
    fireEvent.click(await screen.findByText('Welt'));
    await waitFor(() => expect(onSelectNote).toHaveBeenCalledWith('n2', 'c2'));
  });
});
