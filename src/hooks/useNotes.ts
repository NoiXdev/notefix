import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { Note } from '../types';

const sortNotes = (a: Note, b: Note) =>
  Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt;

export function useNotes() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);

  const reload = useCallback(async () => {
    const loaded = await api.notes.load();
    setNotes(loaded);
  }, []);

  useEffect(() => {
    reload().finally(() => setLoading(false));
    const unsubscribe = api.onNotesChanged(reload);
    return unsubscribe;
  }, [reload]);

  const createNote = useCallback(async (): Promise<string> => {
    const note: Note = { id: crypto.randomUUID(), content: '', updatedAt: Date.now(), pinned: false };
    await api.notes.save(note);
    setNotes(prev => [note, ...prev]);
    return note.id;
  }, []);

  const updateNote = useCallback(async (id: string, content: string) => {
    const updatedAt = Date.now();
    // Local state keeps `pinned` via {...n}; backend's save_note preserves pinned on conflict,
    // so the value sent here is irrelevant.
    setNotes(prev =>
      prev.map(n => (n.id === id ? { ...n, content, updatedAt } : n)).sort(sortNotes),
    );
    await api.notes.save({ id, content, updatedAt, pinned: false });
  }, []);

  const deleteNote = useCallback(async (id: string) => {
    await api.notes.delete(id);
    setNotes(prev => prev.filter(n => n.id !== id));
  }, []);

  const setPinned = useCallback(async (id: string, pinned: boolean) => {
    setNotes(prev => prev.map(n => (n.id === id ? { ...n, pinned } : n)).sort(sortNotes));
    await api.notes.setPinned(id, pinned);
  }, []);

  return { notes, loading, createNote, updateNote, deleteNote, setPinned };
}
