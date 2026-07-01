import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { NoteMeta } from '../types';
import { getPreview } from '../preview';
import { countTasks } from '../tasks';

const sortNotes = (a: NoteMeta, b: NoteMeta) => Number(b.pinned) - Number(a.pinned) || a.position - b.position;

export function useNotes() {
  const [notes, setNotes] = useState<NoteMeta[]>([]);
  const [trashed, setTrashed] = useState<NoteMeta[]>([]);
  const [loading, setLoading] = useState(true);

  const reload = useCallback(async () => {
    const [active, t] = await Promise.all([api.notes.load(), api.trash.load()]);
    setNotes(active);
    setTrashed(t);
  }, []);

  useEffect(() => {
    reload().finally(() => setLoading(false));
    return api.onNotesChanged(reload);
  }, [reload]);

  const createNote = useCallback(async (): Promise<string> => {
    const id = crypto.randomUUID();
    const updatedAt = Date.now();
    const position = -updatedAt;
    await api.notes.save({ id, content: '', updatedAt, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position, deletedAt: null });
    const meta: NoteMeta = { id, updatedAt, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position, deletedAt: null, preview: '', tasksDone: 0, tasksTotal: 0 };
    setNotes(prev => [meta, ...prev]);
    return id;
  }, []);

  const updateNote = useCallback(async (id: string, content: string) => {
    const updatedAt = Date.now();
    // Optimistically refresh preview + task counts so the sidebar stays live
    // while typing; the store round-trip confirms them on the next reload.
    const tasks = countTasks(content);
    setNotes(prev =>
      prev
        .map(n => (n.id === id ? { ...n, updatedAt, preview: getPreview(content), tasksDone: tasks.done, tasksTotal: tasks.total } : n))
        .sort(sortNotes),
    );
    // backend's save_note preserves pinned/archived/color on conflict.
    await api.notes.save({ id, content, updatedAt, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null });
  }, []);

  const reorderNotes = useCallback(async (folderId: string | null, ids: string[]) => {
    setNotes(prev => prev.map(n => {
      const idx = ids.indexOf(n.id);
      return idx === -1 ? n : { ...n, folderId, position: idx };
    }).sort(sortNotes));
    await api.notes.reorder(folderId, ids);
  }, []);

  const deleteNote = useCallback(async (id: string) => {
    setNotes(prev => prev.filter(n => n.id !== id));
    await api.notes.delete(id);
    await reload();
  }, [reload]);

  const setPinned = useCallback(async (id: string, pinned: boolean) => {
    setNotes(prev => prev.map(n => (n.id === id ? { ...n, pinned } : n)).sort(sortNotes));
    await api.notes.setPinned(id, pinned);
  }, []);

  const setArchived = useCallback(async (id: string, archived: boolean) => {
    setNotes(prev => prev.map(n => (n.id === id ? { ...n, archived } : n)));
    await api.notes.setArchived(id, archived);
  }, []);

  const setColor = useCallback(async (id: string, color: string) => {
    setNotes(prev => prev.map(n => (n.id === id ? { ...n, color } : n)));
    await api.notes.setColor(id, color);
  }, []);

  const setDue = useCallback(async (id: string, dueAt: number | null) => {
    setNotes(prev => prev.map(n => (n.id === id ? { ...n, dueAt } : n)));
    await api.notes.setDue(id, dueAt);
  }, []);

  const setFolder = useCallback(async (id: string, folderId: string | null) => {
    setNotes(prev => prev.map(n => (n.id === id ? { ...n, folderId } : n)));
    await api.notes.setFolder(id, folderId);
  }, []);

  const restoreNote = useCallback(async (id: string) => { await api.notes.restore(id); await reload(); }, [reload]);
  const purgeNote = useCallback(async (id: string) => { await api.notes.purge(id); await reload(); }, [reload]);
  const emptyTrash = useCallback(async () => { await api.trash.empty(); await reload(); }, [reload]);

  return { notes, trashed, loading, createNote, updateNote, deleteNote, setPinned, setArchived, setColor, setDue, setFolder, reorderNotes, restoreNote, purgeNote, emptyTrash, reload };
}
