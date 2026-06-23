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
    return api.onNotesChanged(reload); // folder mutations broadcast notes-changed
  }, [reload]);

  const createFolder = useCallback(async (name: string, parentId: string | null) => {
    await api.folders.create(crypto.randomUUID(), name, parentId);
  }, []);
  const renameFolder = useCallback((id: string, name: string) => api.folders.rename(id, name), []);
  const moveFolder = useCallback((id: string, parentId: string | null) => api.folders.move(id, parentId), []);
  const deleteFolder = useCallback((id: string, mode: 'reparent' | 'recursive') => api.folders.delete(id, mode), []);

  return { folders, createFolder, renameFolder, moveFolder, deleteFolder };
}
