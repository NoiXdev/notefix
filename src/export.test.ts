import { describe, it, expect, vi, beforeEach } from 'vitest';

const { saveMock, openMock, exportNotesBase64Mock, exportNotesBundleMock } = vi.hoisted(() => ({
  saveMock: vi.fn(),
  openMock: vi.fn(),
  exportNotesBase64Mock: vi.fn(() => Promise.resolve()),
  exportNotesBundleMock: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ save: saveMock, open: openMock }));
vi.mock('./api', () => ({
  api: {
    exportNotesBase64: exportNotesBase64Mock,
    exportNotesBundle: exportNotesBundleMock,
  },
}));

import { exportBase64, exportBundle } from './export';

beforeEach(() => {
  vi.clearAllMocks();
});

describe('exportBase64', () => {
  it('opens a save dialog for a JSON file and forwards path + ids to api.exportNotesBase64', async () => {
    saveMock.mockResolvedValueOnce('/Users/me/backup.json');

    await exportBase64(['n1', 'n2'], 'backup.json');

    expect(saveMock).toHaveBeenCalledExactlyOnceWith({
      defaultPath: 'backup.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });
    expect(exportNotesBase64Mock).toHaveBeenCalledExactlyOnceWith('/Users/me/backup.json', ['n1', 'n2']);
  });

  it('does nothing when the save dialog is cancelled', async () => {
    saveMock.mockResolvedValueOnce(null);

    await exportBase64(['n1'], 'backup.json');

    expect(exportNotesBase64Mock).not.toHaveBeenCalled();
  });
});

describe('exportBundle', () => {
  it('opens a directory dialog and forwards dir + ids to api.exportNotesBundle', async () => {
    openMock.mockResolvedValueOnce('/Users/me/notes-export');

    await exportBundle(['n1', 'n2', 'n3']);

    expect(openMock).toHaveBeenCalledExactlyOnceWith({ directory: true });
    expect(exportNotesBundleMock).toHaveBeenCalledExactlyOnceWith('/Users/me/notes-export', ['n1', 'n2', 'n3']);
  });

  it('does nothing when the directory dialog is cancelled (null result)', async () => {
    openMock.mockResolvedValueOnce(null);

    await exportBundle(['n1']);

    expect(exportNotesBundleMock).not.toHaveBeenCalled();
  });

  it('does nothing when the dialog resolves to a non-string (e.g. an array)', async () => {
    openMock.mockResolvedValueOnce(['/a', '/b']);

    await exportBundle(['n1']);

    expect(exportNotesBundleMock).not.toHaveBeenCalled();
  });
});
