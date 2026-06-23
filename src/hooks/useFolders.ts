import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { Folder } from '../types';

export function useFolders() {
  const [folders, setFolders] = useState<Folder[]>([]);

  const reload = useCallback(async () => {
    setFolders(await api.folders.load());
  }, []);

  useEffect(() => {
    reload();
    return api.onNotesChanged(reload);
  }, [reload]);

  // The `notes-changed` broadcast skips the sender window, so the window that
  // performs a folder mutation must refresh its own list explicitly.
  const createFolder = useCallback(async (name: string, parentId: string | null): Promise<string> => {
    const id = crypto.randomUUID();
    await api.folders.create(id, name, parentId);
    await reload();
    return id;
  }, [reload]);

  const renameFolder = useCallback(async (id: string, name: string) => {
    await api.folders.rename(id, name);
    await reload();
  }, [reload]);

  const moveFolder = useCallback(async (id: string, parentId: string | null) => {
    await api.folders.move(id, parentId);
    await reload();
  }, [reload]);

  const deleteFolder = useCallback(async (id: string, mode: 'reparent' | 'recursive') => {
    await api.folders.delete(id, mode);
    await reload();
  }, [reload]);

  return { folders, createFolder, renameFolder, moveFolder, deleteFolder };
}
