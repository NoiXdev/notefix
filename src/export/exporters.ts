import { save, open } from '@tauri-apps/plugin-dialog';
import { api } from '../api';
import { htmlToMarkdown } from '../markdown';
import { getPreview } from '../preview';
import { htmlToText, wordHtml } from './docHtml';
import { htmlToPdf, htmlToJpg } from './render';
import type { Note } from '../types';

export type ExportFormat = 'md' | 'txt' | 'pdf' | 'jpg' | 'doc';

const EXT: Record<ExportFormat, { ext: string; name: string }> = {
  md: { ext: 'md', name: 'Markdown' },
  txt: { ext: 'txt', name: 'Text' },
  pdf: { ext: 'pdf', name: 'PDF' },
  jpg: { ext: 'jpg', name: 'JPEG' },
  doc: { ext: 'doc', name: 'Word' },
};

function baseName(note: Note): string {
  return (getPreview(note.content).slice(0, 40) || 'note').replace(/[/\\:]/g, '-');
}

export async function exportNote(note: Note, format: ExportFormat, mdBundle: boolean): Promise<void> {
  const name = baseName(note);

  if (format === 'md' && mdBundle) {
    const dir = await open({ directory: true });
    if (typeof dir !== 'string') return;
    await api.exportMdBundle(dir, htmlToMarkdown(note.content), name);
    return;
  }

  const f = EXT[format];
  const path = await save({ defaultPath: `${name}.${f.ext}`, filters: [{ name: f.name, extensions: [f.ext] }] });
  if (!path) return;

  const inlined = await api.noteInlinedHtml(note.id);
  let bytes: number[];
  if (format === 'md') bytes = Array.from(new TextEncoder().encode(htmlToMarkdown(inlined)));
  else if (format === 'txt') bytes = Array.from(new TextEncoder().encode(htmlToText(inlined)));
  else if (format === 'doc') bytes = Array.from(new TextEncoder().encode(wordHtml(name, inlined)));
  else if (format === 'pdf') bytes = Array.from(await htmlToPdf(inlined));
  else bytes = Array.from(await htmlToJpg(inlined));

  await api.saveExport(path, bytes);
}
