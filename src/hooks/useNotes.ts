import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { Note } from '../types';

const sortNotes = (a: Note, b: Note) => Number(b.pinned) - Number(a.pinned) || a.position - b.position;

export function useNotes() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);

  const reload = useCallback(async () => {
    setNotes(await api.notes.load());
  }, []);

  useEffect(() => {
    reload().finally(() => setLoading(false));
    return api.onNotesChanged(reload);
  }, [reload]);

  const createNote = useCallback(async (): Promise<string> => {
    const note: Note = {
      id: crypto.randomUUID(), content: '', updatedAt: Date.now(),
      pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: -Date.now(),
    };
    await api.notes.save(note);
    setNotes(prev => [note, ...prev]);
    return note.id;
  }, []);

  const updateNote = useCallback(async (id: string, content: string) => {
    const updatedAt = Date.now();
    setNotes(prev =>
      prev.map(n => (n.id === id ? { ...n, content, updatedAt } : n)).sort(sortNotes),
    );
    // backend's save_note preserves pinned/archived/color on conflict.
    await api.notes.save({ id, content, updatedAt, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0 });
  }, []);

  const reorderNotes = useCallback(async (folderId: string | null, ids: string[]) => {
    setNotes(prev => prev.map(n => {
      const idx = ids.indexOf(n.id);
      return idx === -1 ? n : { ...n, folderId, position: idx };
    }).sort(sortNotes));
    await api.notes.reorder(folderId, ids);
  }, []);

  const deleteNote = useCallback(async (id: string) => {
    await api.notes.delete(id);
    setNotes(prev => prev.filter(n => n.id !== id));
  }, []);

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

  return { notes, loading, createNote, updateNote, deleteNote, setPinned, setArchived, setColor, setDue, setFolder, reorderNotes };
}
