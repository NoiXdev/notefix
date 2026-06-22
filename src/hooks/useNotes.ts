import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { Note } from '../types';

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
    const note: Note = { id: crypto.randomUUID(), content: '', updatedAt: Date.now() };
    await api.notes.save(note);
    setNotes(prev => [note, ...prev]);
    return note.id;
  }, []);

  const updateNote = useCallback(async (id: string, content: string) => {
    const updated: Note = { id, content, updatedAt: Date.now() };
    await api.notes.save(updated);
    setNotes(prev =>
      prev
        .map(n => (n.id === id ? updated : n))
        .sort((a, b) => b.updatedAt - a.updatedAt),
    );
  }, []);

  const deleteNote = useCallback(async (id: string) => {
    await api.notes.delete(id);
    setNotes(prev => prev.filter(n => n.id !== id));
  }, []);

  return { notes, loading, createNote, updateNote, deleteNote };
}
