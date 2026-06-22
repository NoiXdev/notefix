import { save } from '@tauri-apps/plugin-dialog';
import { api } from './api';

/** Open a save dialog and write the selected notes (empty ids => all) as JSON. */
export async function exportSelected(ids: string[], suggestedName: string): Promise<void> {
  const path = await save({
    defaultPath: suggestedName,
    filters: [{ name: 'JSON', extensions: ['json'] }],
  });
  if (path) await api.exportNotes(path, ids);
}
