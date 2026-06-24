import { save, open } from '@tauri-apps/plugin-dialog';
import { api } from './api';

/** base64-Einbettung: einzelne JSON-Datei. */
export async function exportBase64(ids: string[], suggestedName: string): Promise<void> {
  const path = await save({ defaultPath: suggestedName, filters: [{ name: 'JSON', extensions: ['json'] }] });
  if (path) await api.exportNotesBase64(path, ids);
}

/** Bundle: Ziel-Ordner mit notes.json + images/. */
export async function exportBundle(ids: string[]): Promise<void> {
  const dir = await open({ directory: true });
  if (typeof dir === 'string') await api.exportNotesBundle(dir, ids);
}
