import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { NoteMeta } from '../types';

const { saveMock, openMock, loadOneMock, exportMdBundleMock, noteInlinedHtmlMock, saveExportMock, htmlToPdfMock, htmlToJpgMock } = vi.hoisted(() => ({
  saveMock: vi.fn(),
  openMock: vi.fn(),
  loadOneMock: vi.fn(),
  exportMdBundleMock: vi.fn(() => Promise.resolve()),
  noteInlinedHtmlMock: vi.fn(),
  saveExportMock: vi.fn(() => Promise.resolve()),
  htmlToPdfMock: vi.fn(),
  htmlToJpgMock: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ save: saveMock, open: openMock }));
vi.mock('../api', () => ({
  api: {
    notes: { loadOne: loadOneMock },
    exportMdBundle: exportMdBundleMock,
    noteInlinedHtml: noteInlinedHtmlMock,
    saveExport: saveExportMock,
  },
}));
// html2canvas/jsPDF need a real canvas backend that jsdom doesn't provide;
// exporters.ts only cares about render.ts's *return value* (the encoded
// bytes), so the render boundary itself is mocked (its own logic is covered
// by render.test.ts).
vi.mock('./render', () => ({ htmlToPdf: htmlToPdfMock, htmlToJpg: htmlToJpgMock }));

import { exportNote, type ExportFormat } from './exporters';

function meta(over: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: 'n1', updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null,
    folderId: null, position: 0, deletedAt: null, preview: 'My note title',
    tasksDone: 0, tasksTotal: 0, protected: false, title: 'My note title', mcpHidden: false,
    ...over,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('exportNote — markdown bundle path (format=md, mdBundle=true)', () => {
  it('opens a directory dialog, loads raw content, converts to markdown, and calls exportMdBundle', async () => {
    openMock.mockResolvedValueOnce('/Users/me/out');
    loadOneMock.mockResolvedValueOnce('<h1>Title</h1><p>Body <strong>text</strong></p>');

    await exportNote(meta(), 'md', true);

    expect(openMock).toHaveBeenCalledExactlyOnceWith({ directory: true });
    expect(loadOneMock).toHaveBeenCalledExactlyOnceWith('n1');
    expect(exportMdBundleMock).toHaveBeenCalledTimes(1);
    const [dir, md, name] = exportMdBundleMock.mock.calls[0];
    expect(dir).toBe('/Users/me/out');
    expect(md).toContain('# Title');
    expect(md).toContain('**text**');
    expect(name).toBe('My note title');
    // The plain (non-bundle) export path must not run.
    expect(noteInlinedHtmlMock).not.toHaveBeenCalled();
    expect(saveExportMock).not.toHaveBeenCalled();
  });

  it('does nothing when the directory dialog is cancelled', async () => {
    openMock.mockResolvedValueOnce(undefined);

    await exportNote(meta(), 'md', true);

    expect(loadOneMock).not.toHaveBeenCalled();
    expect(exportMdBundleMock).not.toHaveBeenCalled();
  });
});

describe('exportNote — file-save path (mdBundle=false, or any non-md format)', () => {
  it('opens a save dialog with the note-derived filename and format filter, per format', async () => {
    const cases: { format: ExportFormat; ext: string; filterName: string }[] = [
      { format: 'md', ext: 'md', filterName: 'Markdown' },
      { format: 'txt', ext: 'txt', filterName: 'Text' },
      { format: 'doc', ext: 'doc', filterName: 'Word' },
    ];
    for (const { format, ext, filterName } of cases) {
      saveMock.mockResolvedValueOnce(`/out/note.${ext}`);
      noteInlinedHtmlMock.mockResolvedValueOnce('<p>x</p>');

      await exportNote(meta(), format, false);

      expect(saveMock).toHaveBeenCalledWith({
        defaultPath: `My note title.${ext}`,
        filters: [{ name: filterName, extensions: [ext] }],
      });
    }
  });

  it('does nothing when the save dialog is cancelled (falsy path)', async () => {
    saveMock.mockResolvedValueOnce(null);

    await exportNote(meta(), 'txt', false);

    expect(noteInlinedHtmlMock).not.toHaveBeenCalled();
    expect(saveExportMock).not.toHaveBeenCalled();
  });

  it('format=md writes UTF-8 bytes of the markdown conversion of the inlined html', async () => {
    saveMock.mockResolvedValueOnce('/out/note.md');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>Hello <em>world</em></p>');

    await exportNote(meta(), 'md', false);

    expect(noteInlinedHtmlMock).toHaveBeenCalledExactlyOnceWith('n1');
    expect(saveExportMock).toHaveBeenCalledTimes(1);
    const [path, bytes] = saveExportMock.mock.calls[0];
    expect(path).toBe('/out/note.md');
    const text = new TextDecoder().decode(new Uint8Array(bytes));
    expect(text).toContain('Hello _world_');
  });

  it('format=txt writes UTF-8 bytes of the plain text extracted from the inlined html', async () => {
    saveMock.mockResolvedValueOnce('/out/note.txt');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>Line one</p><p>Line two</p>');

    await exportNote(meta(), 'txt', false);

    const [, bytes] = saveExportMock.mock.calls[0];
    const text = new TextDecoder().decode(new Uint8Array(bytes));
    expect(text).toBe('Line one\nLine two');
  });

  it('format=doc writes UTF-8 bytes of an Office-namespaced HTML document', async () => {
    saveMock.mockResolvedValueOnce('/out/note.doc');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>Body</p>');

    await exportNote(meta(), 'doc', false);

    const [, bytes] = saveExportMock.mock.calls[0];
    const text = new TextDecoder().decode(new Uint8Array(bytes));
    expect(text).toContain('urn:schemas-microsoft-com:office:word');
    expect(text).toContain('<title>My note title</title>');
    expect(text).toContain('<p>Body</p>');
  });

  it('format=pdf delegates rendering to render.htmlToPdf and writes its bytes', async () => {
    saveMock.mockResolvedValueOnce('/out/note.pdf');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>pdf body</p>');
    htmlToPdfMock.mockResolvedValueOnce(new Uint8Array([9, 8, 7]));

    await exportNote(meta(), 'pdf', false);

    expect(htmlToPdfMock).toHaveBeenCalledExactlyOnceWith('<p>pdf body</p>');
    expect(saveExportMock).toHaveBeenCalledExactlyOnceWith('/out/note.pdf', [9, 8, 7]);
  });

  it('format=jpg delegates rendering to render.htmlToJpg and writes its bytes', async () => {
    saveMock.mockResolvedValueOnce('/out/note.jpg');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>jpg body</p>');
    htmlToJpgMock.mockResolvedValueOnce(new Uint8Array([1, 2, 3]));

    await exportNote(meta(), 'jpg', false);

    expect(htmlToJpgMock).toHaveBeenCalledExactlyOnceWith('<p>jpg body</p>');
    expect(saveExportMock).toHaveBeenCalledExactlyOnceWith('/out/note.jpg', [1, 2, 3]);
  });

  it('format=md with mdBundle=false takes the save-dialog path, not the bundle path', async () => {
    saveMock.mockResolvedValueOnce('/out/note.md');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>x</p>');

    await exportNote(meta(), 'md', false);

    expect(openMock).not.toHaveBeenCalled();
    expect(exportMdBundleMock).not.toHaveBeenCalled();
    expect(saveMock).toHaveBeenCalledTimes(1);
  });
});

describe('exportNote — filename derivation from note.preview', () => {
  it('truncates the preview to 40 characters', async () => {
    const longPreview = 'x'.repeat(80);
    saveMock.mockResolvedValueOnce('/out/whatever.txt');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>x</p>');

    await exportNote(meta({ preview: longPreview }), 'txt', false);

    expect(saveMock).toHaveBeenCalledExactlyOnceWith({
      defaultPath: `${'x'.repeat(40)}.txt`,
      filters: [{ name: 'Text', extensions: ['txt'] }],
    });
  });

  it('replaces path-unsafe characters (/, \\, :) with a dash', async () => {
    saveMock.mockResolvedValueOnce('/out/whatever.txt');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>x</p>');

    await exportNote(meta({ preview: 'a/b\\c:d' }), 'txt', false);

    expect(saveMock).toHaveBeenCalledExactlyOnceWith({
      defaultPath: 'a-b-c-d.txt',
      filters: [{ name: 'Text', extensions: ['txt'] }],
    });
  });

  it('falls back to "note" when the preview is empty', async () => {
    saveMock.mockResolvedValueOnce('/out/note.txt');
    noteInlinedHtmlMock.mockResolvedValueOnce('<p>x</p>');

    await exportNote(meta({ preview: '' }), 'txt', false);

    expect(saveMock).toHaveBeenCalledExactlyOnceWith({
      defaultPath: 'note.txt',
      filters: [{ name: 'Text', extensions: ['txt'] }],
    });
  });
});
