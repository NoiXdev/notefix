import { describe, it, expect, vi, beforeEach } from 'vitest';

// `vi.mock` factories are hoisted above imports, so the mock fns they close
// over must be created via `vi.hoisted` rather than plain `const` above them.
const {
  invokeMock,
  listenMock,
  getVersionMock,
  openUrlMock,
  autostartIsEnabledMock,
  autostartEnableMock,
  autostartDisableMock,
  processRelaunchMock,
  openDialogMock,
  setTitleMock,
  setAlwaysOnTopMock,
  closeMock,
  startResizeDraggingMock,
  titleMock,
  getCurrentWindowMock,
} = vi.hoisted(() => {
  const setTitleMock = vi.fn(() => Promise.resolve());
  const setAlwaysOnTopMock = vi.fn(() => Promise.resolve());
  const closeMock = vi.fn(() => Promise.resolve());
  const startResizeDraggingMock = vi.fn(() => Promise.resolve());
  const titleMock = vi.fn(() => Promise.resolve('Notefix'));
  return {
    invokeMock: vi.fn(),
    listenMock: vi.fn(() => Promise.resolve(vi.fn())),
    getVersionMock: vi.fn(() => Promise.resolve('1.2.3')),
    openUrlMock: vi.fn(() => Promise.resolve()),
    autostartIsEnabledMock: vi.fn(() => Promise.resolve(false)),
    autostartEnableMock: vi.fn(() => Promise.resolve()),
    autostartDisableMock: vi.fn(() => Promise.resolve()),
    processRelaunchMock: vi.fn(() => Promise.resolve()),
    openDialogMock: vi.fn(() => Promise.resolve(null)),
    setTitleMock,
    setAlwaysOnTopMock,
    closeMock,
    startResizeDraggingMock,
    titleMock,
    getCurrentWindowMock: vi.fn(() => ({
      setTitle: setTitleMock,
      setAlwaysOnTop: setAlwaysOnTopMock,
      close: closeMock,
      startResizeDragging: startResizeDraggingMock,
      title: titleMock,
    })),
  };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: getCurrentWindowMock }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: openUrlMock }));
vi.mock('@tauri-apps/plugin-autostart', () => ({
  isEnabled: autostartIsEnabledMock,
  enable: autostartEnableMock,
  disable: autostartDisableMock,
}));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: processRelaunchMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openDialogMock }));

import { api } from './api';
import type { Note, NoteMeta } from './types';

beforeEach(() => {
  vi.clearAllMocks();
  listenMock.mockImplementation(() => Promise.resolve(vi.fn()));
  getVersionMock.mockResolvedValue('1.2.3');
  titleMock.mockResolvedValue('Notefix');
  setTitleMock.mockResolvedValue(undefined);
  setAlwaysOnTopMock.mockResolvedValue(undefined);
  closeMock.mockResolvedValue(undefined);
  startResizeDraggingMock.mockResolvedValue(undefined);
});

/** Resolves `invoke` once with `resolved`, invokes `call`, and asserts both
 * the exact `invoke("<command>", <args>)` call and the wrapper's return
 * value (mapped, when the wrapper transforms the raw resolved value). */
async function expectInvoke(
  call: () => Promise<unknown>,
  command: string,
  args: Record<string, unknown> | undefined,
  resolved: unknown,
  expected: unknown = resolved,
): Promise<void> {
  invokeMock.mockResolvedValueOnce(resolved);
  const result = await call();
  // Arg-less wrappers call `invoke("cmd")` with arity 1, not `invoke("cmd",
  // undefined)` — toHaveBeenCalledWith is arity-sensitive, so branch on it.
  if (args === undefined) {
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith(command);
  } else {
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith(command, args);
  }
  expect(result).toEqual(expected);
}

const note: Note = {
  id: 'n1',
  content: '<p>hi</p>',
  updatedAt: 100,
  pinned: false,
  archived: false,
  color: '',
  dueAt: null,
  folderId: null,
  position: 0,
  deletedAt: null,
};

const noteMeta: NoteMeta = {
  id: 'n1',
  updatedAt: 100,
  pinned: false,
  archived: false,
  color: '',
  dueAt: null,
  folderId: null,
  position: 0,
  deletedAt: null,
  preview: 'hi',
  tasksDone: 0,
  tasksTotal: 0,
  protected: false,
  title: 'hi',
  mcpHidden: false,
};

describe('api.notes', () => {
  it('load() calls notes_load with no args', async () => {
    await expectInvoke(() => api.notes.load(), 'notes_load', undefined, [noteMeta]);
  });

  it('loadOne(id) calls notes_load_one with { id }', async () => {
    await expectInvoke(() => api.notes.loadOne('n1'), 'notes_load_one', { id: 'n1' }, '<p>hi</p>');
  });

  it('search(query) calls notes_search with { query }', async () => {
    await expectInvoke(
      () => api.notes.search('foo'),
      'notes_search',
      { query: 'foo' },
      [{ note: noteMeta, snippet: 'hi' }],
    );
  });

  it('searchAll(query) calls notes_search_all with { query }', async () => {
    await expectInvoke(
      () => api.notes.searchAll('foo'),
      'notes_search_all',
      { query: 'foo' },
      [{ contextId: 'c1', contextLabel: 'Local', kind: 'local', note: noteMeta, snippet: 'hi' }],
    );
  });

  it('save(note) calls notes_save with the exact note object', async () => {
    await expectInvoke(() => api.notes.save(note), 'notes_save', { note }, undefined);
  });

  it('delete(id) calls notes_delete with { id }', async () => {
    await expectInvoke(() => api.notes.delete('n1'), 'notes_delete', { id: 'n1' }, undefined);
  });

  it('setPinned(id, pinned) calls notes_set_pinned with { id, pinned }', async () => {
    await expectInvoke(
      () => api.notes.setPinned('n1', true),
      'notes_set_pinned',
      { id: 'n1', pinned: true },
      undefined,
    );
  });

  it('setArchived(id, archived) calls notes_set_archived with { id, archived }', async () => {
    await expectInvoke(
      () => api.notes.setArchived('n1', true),
      'notes_set_archived',
      { id: 'n1', archived: true },
      undefined,
    );
  });

  it('setColor(id, color) calls notes_set_color with { id, color }', async () => {
    await expectInvoke(
      () => api.notes.setColor('n1', '#fff'),
      'notes_set_color',
      { id: 'n1', color: '#fff' },
      undefined,
    );
  });

  it('setDue(id, dueAt) calls notes_set_due with { id, dueAt }', async () => {
    await expectInvoke(
      () => api.notes.setDue('n1', 123),
      'notes_set_due',
      { id: 'n1', dueAt: 123 },
      undefined,
    );
  });

  it('setDue(id, null) passes dueAt: null', async () => {
    await expectInvoke(
      () => api.notes.setDue('n1', null),
      'notes_set_due',
      { id: 'n1', dueAt: null },
      undefined,
    );
  });

  it('setFolder(id, folderId) calls notes_set_folder with { id, folderId }', async () => {
    await expectInvoke(
      () => api.notes.setFolder('n1', 'f1'),
      'notes_set_folder',
      { id: 'n1', folderId: 'f1' },
      undefined,
    );
  });

  it('reorder(folderId, ids) calls notes_reorder with { folderId, ids }', async () => {
    await expectInvoke(
      () => api.notes.reorder('f1', ['a', 'b']),
      'notes_reorder',
      { folderId: 'f1', ids: ['a', 'b'] },
      undefined,
    );
  });

  it('revisions(noteId) calls note_revisions with { noteId }', async () => {
    await expectInvoke(
      () => api.notes.revisions('n1'),
      'note_revisions',
      { noteId: 'n1' },
      [{ id: 1, noteId: 'n1', createdAt: 5 }],
    );
  });

  it('revisionContent(id) calls note_revision_content with { id }', async () => {
    await expectInvoke(
      () => api.notes.revisionContent(1),
      'note_revision_content',
      { id: 1 },
      '<p>old</p>',
    );
  });

  it('restore(id) calls notes_restore with { id }', async () => {
    await expectInvoke(() => api.notes.restore('n1'), 'notes_restore', { id: 'n1' }, undefined);
  });

  it('purge(id) calls notes_purge with { id }', async () => {
    await expectInvoke(() => api.notes.purge('n1'), 'notes_purge', { id: 'n1' }, undefined);
  });

  it('loadAll() calls notes_load_all with no args', async () => {
    await expectInvoke(() => api.notes.loadAll(), 'notes_load_all', undefined, []);
  });

  it('setMcpHidden(id, hidden) calls note_set_mcp_hidden with { id, hidden }', async () => {
    await expectInvoke(
      () => api.notes.setMcpHidden('n1', true),
      'note_set_mcp_hidden',
      { id: 'n1', hidden: true },
      undefined,
    );
  });
});

describe('api.trash', () => {
  it('load() calls trash_load with no args', async () => {
    await expectInvoke(() => api.trash.load(), 'trash_load', undefined, [noteMeta]);
  });

  it('empty() calls trash_empty with no args', async () => {
    await expectInvoke(() => api.trash.empty(), 'trash_empty', undefined, undefined);
  });
});

describe('api.folders', () => {
  it('load() calls folders_load with no args', async () => {
    await expectInvoke(() => api.folders.load(), 'folders_load', undefined, []);
  });

  it('create(id, name, parentId) calls folder_create with { id, name, parentId }', async () => {
    await expectInvoke(
      () => api.folders.create('f1', 'Work', null),
      'folder_create',
      { id: 'f1', name: 'Work', parentId: null },
      undefined,
    );
  });

  it('rename(id, name) calls folder_rename with { id, name }', async () => {
    await expectInvoke(
      () => api.folders.rename('f1', 'Renamed'),
      'folder_rename',
      { id: 'f1', name: 'Renamed' },
      undefined,
    );
  });

  it('move(id, parentId) calls folder_move with { id, parentId }', async () => {
    await expectInvoke(
      () => api.folders.move('f1', 'f2'),
      'folder_move',
      { id: 'f1', parentId: 'f2' },
      undefined,
    );
  });

  it('delete(id, mode) calls folder_delete with { id, mode }', async () => {
    await expectInvoke(
      () => api.folders.delete('f1', 'recursive'),
      'folder_delete',
      { id: 'f1', mode: 'recursive' },
      undefined,
    );
  });

  it('reorder(parentId, ids) calls folders_reorder with { parentId, ids }', async () => {
    await expectInvoke(
      () => api.folders.reorder(null, ['f1', 'f2']),
      'folders_reorder',
      { parentId: null, ids: ['f1', 'f2'] },
      undefined,
    );
  });

  it('setIcon(id, icon) calls folder_set_icon with { id, icon }', async () => {
    await expectInvoke(
      () => api.folders.setIcon('f1', 'star'),
      'folder_set_icon',
      { id: 'f1', icon: 'star' },
      undefined,
    );
  });

  it('setColor(id, color) calls folder_set_color with { id, color }', async () => {
    await expectInvoke(
      () => api.folders.setColor('f1', '#000'),
      'folder_set_color',
      { id: 'f1', color: '#000' },
      undefined,
    );
  });

  it('setSort(id, sort) calls folder_set_sort with { id, sort }', async () => {
    await expectInvoke(
      () => api.folders.setSort('f1', 'name'),
      'folder_set_sort',
      { id: 'f1', sort: 'name' },
      undefined,
    );
  });

  it('setMcpHidden(id, hidden) calls folder_set_mcp_hidden with { id, hidden }', async () => {
    await expectInvoke(
      () => api.folders.setMcpHidden('f1', true),
      'folder_set_mcp_hidden',
      { id: 'f1', hidden: true },
      undefined,
    );
  });
});

describe('api.settings', () => {
  it('load() calls settings_load and maps the tuple array into an object', async () => {
    invokeMock.mockResolvedValueOnce([
      ['theme', 'dark'],
      ['lang', 'de'],
    ]);
    const result = await api.settings.load();
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith('settings_load');
    expect(result).toEqual({ theme: 'dark', lang: 'de' });
  });

  it('load() with an empty tuple array returns an empty object', async () => {
    invokeMock.mockResolvedValueOnce([]);
    const result = await api.settings.load();
    expect(result).toEqual({});
  });

  it('set(key, value) calls settings_set with { key, value }', async () => {
    await expectInvoke(
      () => api.settings.set('theme', 'dark'),
      'settings_set',
      { key: 'theme', value: 'dark' },
      undefined,
    );
  });
});

describe('api.contexts', () => {
  const ctx = {
    id: 'c1', label: 'Local', kind: 'local' as const, path: '/db',
    serverUrl: '', workspaceId: '', active: true,
    vaultExists: false, vaultBiometric: false,
  };

  it('list() calls contexts_list with no args', async () => {
    await expectInvoke(() => api.contexts.list(), 'contexts_list', undefined, [ctx]);
  });

  it('add(label) calls context_add with { label }', async () => {
    await expectInvoke(() => api.contexts.add('Personal'), 'context_add', { label: 'Personal' }, [ctx]);
  });

  it('switch(id) calls context_switch with { id }', async () => {
    await expectInvoke(() => api.contexts.switch('c1'), 'context_switch', { id: 'c1' }, undefined);
  });

  it('rename(id, label) calls context_rename with { id, label }', async () => {
    await expectInvoke(
      () => api.contexts.rename('c1', 'New label'),
      'context_rename',
      { id: 'c1', label: 'New label' },
      [ctx],
    );
  });

  it('remove(id, deleteFile) calls context_remove with { id, deleteFile }', async () => {
    await expectInvoke(
      () => api.contexts.remove('c1', true),
      'context_remove',
      { id: 'c1', deleteFile: true },
      [],
    );
  });

  it('serverAuthBegin(serverUrl) calls server_auth_begin with { serverUrl }', async () => {
    await expectInvoke(
      () => api.contexts.serverAuthBegin('https://example.com'),
      'server_auth_begin',
      { serverUrl: 'https://example.com' },
      'https://example.com/authorize?state=xyz',
    );
  });

  it('serverAuthComplete(url) calls server_auth_complete with { url }', async () => {
    await expectInvoke(
      () => api.contexts.serverAuthComplete('notefix://auth?code=1&state=xyz'),
      'server_auth_complete',
      { url: 'notefix://auth?code=1&state=xyz' },
      [ctx],
    );
  });

  it('serverWorkspaces() calls server_workspaces with no args', async () => {
    await expectInvoke(
      () => api.contexts.serverWorkspaces(),
      'server_workspaces',
      undefined,
      [{ id: 'w1', name: 'Team', role: 'admin' }],
    );
  });

  it('bindWorkspace(id, workspaceId, label) calls context_bind_workspace with the exact args', async () => {
    await expectInvoke(
      () => api.contexts.bindWorkspace('c1', 'w1', 'Team'),
      'context_bind_workspace',
      { id: 'c1', workspaceId: 'w1', label: 'Team' },
      [ctx],
    );
  });

  it('syncNow() calls sync_now with no args', async () => {
    await expectInvoke(() => api.contexts.syncNow(), 'sync_now', undefined, undefined);
  });

  it('syncStatus() calls sync_status with no args', async () => {
    await expectInvoke(
      () => api.contexts.syncStatus(),
      'sync_status',
      undefined,
      { state: 'synced', lastSyncedAt: 42, pending: 0 },
    );
  });

  it('vaultChangePassphrase(id, current, next) calls context_vault_change_passphrase with the exact args', async () => {
    await expectInvoke(
      () => api.contexts.vaultChangePassphrase('c1', 'old', 'new'),
      'context_vault_change_passphrase',
      { id: 'c1', current: 'old', next: 'new' },
      undefined,
    );
  });

  it('vaultInviteResolve(reference) calls vault_invite_resolve with { reference }', async () => {
    await expectInvoke(
      () => api.contexts.vaultInviteResolve('https://s.example.com/invite/tok'),
      'vault_invite_resolve',
      { reference: 'https://s.example.com/invite/tok' },
      7,
    );
  });

  it('vaultInviteShare(invitationId) calls vault_invite_share with { invitationId }', async () => {
    await expectInvoke(
      () => api.contexts.vaultInviteShare(7),
      'vault_invite_share',
      { invitationId: 7 },
      'ABCDE-FGHJK',
    );
  });

  it('vaultInviteAccept(id, code, passphrase) calls vault_invite_accept with the exact args', async () => {
    await expectInvoke(
      () => api.contexts.vaultInviteAccept(7, 'c', 'p'),
      'vault_invite_accept',
      { invitationId: 7, code: 'c', passphrase: 'p' },
      undefined,
    );
  });

  it('vaultInviteRecode() calls vault_invite_recode with no args and resolves the array', async () => {
    await expectInvoke(
      () => api.contexts.vaultInviteRecode(),
      'vault_invite_recode',
      undefined,
      [{ invitationId: 5, code: 'AAAA-1111' }, { invitationId: 6, code: 'BBBB-2222' }],
    );
  });
});

describe('api.vault', () => {
  const status = { exists: true, unlocked: false, biometric: true };

  it('status() calls vault_status with no args', async () => {
    await expectInvoke(() => api.vault.status(), 'vault_status', undefined, status);
  });

  it('setup(passphrase) calls vault_setup with { passphrase }', async () => {
    await expectInvoke(
      () => api.vault.setup('correct horse'),
      'vault_setup',
      { passphrase: 'correct horse' },
      ['aaaa', 'bbbb', 'cccc'],
    );
  });

  it('unlock(passphrase) calls vault_unlock with { passphrase }', async () => {
    await expectInvoke(
      () => api.vault.unlock('correct horse'),
      'vault_unlock',
      { passphrase: 'correct horse' },
      undefined,
    );
  });

  it('unlockRecovery(recovery) calls vault_unlock_recovery with { recovery }', async () => {
    await expectInvoke(
      () => api.vault.unlockRecovery('aaaa-bbbb-cccc'),
      'vault_unlock_recovery',
      { recovery: 'aaaa-bbbb-cccc' },
      undefined,
    );
  });

  it('unlockBiometric() calls vault_unlock_biometric with no args', async () => {
    await expectInvoke(() => api.vault.unlockBiometric(), 'vault_unlock_biometric', undefined, undefined);
  });

  it('lock() calls vault_lock with no args', async () => {
    await expectInvoke(() => api.vault.lock(), 'vault_lock', undefined, undefined);
  });

  it('changePassphrase(current, next) calls vault_change_passphrase with { current, next }', async () => {
    await expectInvoke(
      () => api.vault.changePassphrase('old', 'new'),
      'vault_change_passphrase',
      { current: 'old', next: 'new' },
      undefined,
    );
  });

  it('rotate(passphrase, recoveryKey) calls vault_rotate and returns the one-time codes', async () => {
    await expectInvoke(
      () => api.vault.rotate('pw', 'AAAAA-BBBBB'),
      'vault_rotate',
      { passphrase: 'pw', recoveryKey: 'AAAAA-BBBBB' },
      [{ userId: 2, code: 'CCCCC-DDDDD' }],
    );
  });

  it('rotate() without a recovery key sends an explicit null', async () => {
    await expectInvoke(
      () => api.vault.rotate('pw'),
      'vault_rotate',
      { passphrase: 'pw', recoveryKey: null },
      [],
    );
  });

  it('rotationRedeem(code, passphrase) calls vault_rotation_redeem with the exact args', async () => {
    await expectInvoke(
      () => api.vault.rotationRedeem('AAAA-BBBB', 'pw'),
      'vault_rotation_redeem',
      { code: 'AAAA-BBBB', passphrase: 'pw' },
      undefined,
    );
  });

  it('recoveryFollowup(recoveryKey) calls vault_recovery_followup with { recoveryKey }', async () => {
    await expectInvoke(
      () => api.vault.recoveryFollowup('AAAAA-BBBBB'),
      'vault_recovery_followup',
      { recoveryKey: 'AAAAA-BBBBB' },
      undefined,
    );
  });

  it('recoveryCreate() calls vault_recovery_create with no args', async () => {
    await expectInvoke(
      () => api.vault.recoveryCreate(),
      'vault_recovery_create',
      undefined,
      { groups: ['AAAAA', 'BBBBB', 'CCCCC'], incomplete: false },
    );
  });

  it('vault.resolveConflict passes both secrets and the mode to vault_resolve_conflict', async () => {
    await expectInvoke(
      () => api.vault.resolveConflict('ws', { kind: 'recovery', value: 'KEY' }, 'unprotect'),
      'vault_resolve_conflict',
      { workspacePassphrase: 'ws', localSecret: { kind: 'recovery', value: 'KEY' }, mode: 'unprotect' },
      { changed: 1, skipped: 0 },
    );
  });

  it('biometricAvailable() calls vault_biometric_available with no args', async () => {
    await expectInvoke(() => api.vault.biometricAvailable(), 'vault_biometric_available', undefined, true);
  });

  it('biometricEnable() calls vault_biometric_enable with no args', async () => {
    await expectInvoke(() => api.vault.biometricEnable(), 'vault_biometric_enable', undefined, undefined);
  });

  it('biometricDisable() calls vault_biometric_disable with no args', async () => {
    await expectInvoke(() => api.vault.biometricDisable(), 'vault_biometric_disable', undefined, undefined);
  });

  it('protectNote(id, isProtected) calls note_set_protected with { id, protected: isProtected }', async () => {
    // The Rust parameter is named `protected` (a reserved-ish word avoided in
    // the JS param name `isProtected`) — the invoke arg key must still be
    // the literal string "protected" to match the Rust signature.
    await expectInvoke(
      () => api.vault.protectNote('n1', true),
      'note_set_protected',
      { id: 'n1', protected: true },
      undefined,
    );
  });

  it('protectNote(id, false) sends protected: false', async () => {
    await expectInvoke(
      () => api.vault.protectNote('n1', false),
      'note_set_protected',
      { id: 'n1', protected: false },
      undefined,
    );
  });

  it('lockFolder(id, locked) calls folder_set_locked with { id, locked }', async () => {
    await expectInvoke(
      () => api.vault.lockFolder('f1', true),
      'folder_set_locked',
      { id: 'f1', locked: true },
      undefined,
    );
  });
});

describe('api top-level wrappers', () => {
  it('saveImage(noteId, name, bytes) calls save_image with the exact args', async () => {
    await expectInvoke(
      () => api.saveImage('n1', 'pic.png', [1, 2, 3]),
      'save_image',
      { noteId: 'n1', name: 'pic.png', bytes: [1, 2, 3] },
      'noteimg://localhost/n/1/pic.png',
    );
  });

  it('exportNotes(path, ids) calls export_notes with { path, ids }', async () => {
    await expectInvoke(
      () => api.exportNotes('/out.json', ['n1', 'n2']),
      'export_notes',
      { path: '/out.json', ids: ['n1', 'n2'] },
      undefined,
    );
  });

  it('exportNotesBase64(path, ids) calls export_notes_base64 with { path, ids }', async () => {
    await expectInvoke(
      () => api.exportNotesBase64('/out.json', ['n1']),
      'export_notes_base64',
      { path: '/out.json', ids: ['n1'] },
      undefined,
    );
  });

  it('exportNotesBundle(dir, ids) calls export_notes_bundle with { dir, ids }', async () => {
    await expectInvoke(
      () => api.exportNotesBundle('/out', ['n1']),
      'export_notes_bundle',
      { dir: '/out', ids: ['n1'] },
      undefined,
    );
  });

  it('noteInlinedHtml(noteId) calls note_inlined_html with { noteId }', async () => {
    await expectInvoke(
      () => api.noteInlinedHtml('n1'),
      'note_inlined_html',
      { noteId: 'n1' },
      '<p>inlined</p>',
    );
  });

  it('saveExport(path, bytes) calls save_export with { path, bytes }', async () => {
    await expectInvoke(
      () => api.saveExport('/out.pdf', [1, 2]),
      'save_export',
      { path: '/out.pdf', bytes: [1, 2] },
      undefined,
    );
  });

  it('exportMdBundle(dir, md, name) calls export_md_bundle with { dir, md, name }', async () => {
    await expectInvoke(
      () => api.exportMdBundle('/out', '# Title', 'note'),
      'export_md_bundle',
      { dir: '/out', md: '# Title', name: 'note' },
      undefined,
    );
  });

  it('stats() calls note_stats with no args', async () => {
    await expectInvoke(
      () => api.stats(),
      'note_stats',
      undefined,
      { notes: 1, archived: 0, characters: 2, words: 1 },
    );
  });

  it('getDbPath() calls get_db_path with no args', async () => {
    await expectInvoke(() => api.getDbPath(), 'get_db_path', undefined, '/db/notefix.db');
  });

  it('setDbLocation(folder) calls set_db_location with { folder }', async () => {
    await expectInvoke(
      () => api.setDbLocation('/new'),
      'set_db_location',
      { folder: '/new' },
      { mode: 'moved', path: '/new/notefix.db' },
    );
  });

  it('quitApp() calls quit_app with no args', async () => {
    await expectInvoke(() => api.quitApp(), 'quit_app', undefined, undefined);
  });

  it('hideMain() calls hide_main with no args', async () => {
    await expectInvoke(() => api.hideMain(), 'hide_main', undefined, undefined);
  });

  it('checkPaths() calls check_paths with no args', async () => {
    await expectInvoke(
      () => api.checkPaths(),
      'check_paths',
      undefined,
      { dbWritable: true, imagesWritable: true, dbPath: '/db', imagesPath: '/db/images' },
    );
  });

  it('openNoteWindow(noteId) calls open_note_window with { noteId }', async () => {
    await expectInvoke(
      () => api.openNoteWindow('n1'),
      'open_note_window',
      { noteId: 'n1' },
      undefined,
    );
  });

  it('fetchLinkMeta(url) calls fetch_link_meta with { url }', async () => {
    await expectInvoke(
      () => api.fetchLinkMeta('https://example.com'),
      'fetch_link_meta',
      { url: 'https://example.com' },
      { url: 'https://example.com', title: 'T', description: '', image: '', site: 'example.com' },
    );
  });

  it('checkForUpdate() calls check_for_update with no args', async () => {
    await expectInvoke(
      () => api.checkForUpdate(),
      'check_for_update',
      undefined,
      { current: '1.0.0', latest: '1.1.0', updateAvailable: true, url: 'https://x' },
    );
  });

  it('githubReleases() calls github_releases with no args', async () => {
    await expectInvoke(
      () => api.githubReleases(),
      'github_releases',
      undefined,
      [{ tagName: 'v1.1.0', name: 'v1.1.0', body: '', publishedAt: '2026-01-01', prerelease: false }],
    );
  });

  it('mcpApplyConfig(c) calls mcp_apply_config with the exact config object', async () => {
    const config = { enabled: true, bind: '127.0.0.1', port: 8790, token: 'tok', authRequired: true, allowWrite: false };
    await expectInvoke(() => api.mcpApplyConfig(config), 'mcp_apply_config', config, undefined);
  });
});

describe('api relaunch / pickFolder / autostart', () => {
  it('relaunch() delegates to the process plugin', async () => {
    await api.relaunch();
    expect(processRelaunchMock).toHaveBeenCalledTimes(1);
  });

  it('pickFolder() opens a directory dialog and returns the picked path', async () => {
    openDialogMock.mockResolvedValueOnce('/Users/me/Notes');
    const result = await api.pickFolder();
    expect(openDialogMock).toHaveBeenCalledExactlyOnceWith({ directory: true });
    expect(result).toBe('/Users/me/Notes');
  });

  it('pickFolder() returns null when the dialog is cancelled (non-string result)', async () => {
    openDialogMock.mockResolvedValueOnce(null);
    const result = await api.pickFolder();
    expect(result).toBeNull();
  });

  it('pickFolder() returns null for an array result (multiple-selection shape)', async () => {
    openDialogMock.mockResolvedValueOnce(['/a', '/b']);
    const result = await api.pickFolder();
    expect(result).toBeNull();
  });

  it('autostart.isEnabled() delegates to the autostart plugin', async () => {
    autostartIsEnabledMock.mockResolvedValueOnce(true);
    expect(await api.autostart.isEnabled()).toBe(true);
    expect(autostartIsEnabledMock).toHaveBeenCalledTimes(1);
  });

  it('autostart.enable() delegates to the autostart plugin', async () => {
    await api.autostart.enable();
    expect(autostartEnableMock).toHaveBeenCalledTimes(1);
  });

  it('autostart.disable() delegates to the autostart plugin', async () => {
    await api.autostart.disable();
    expect(autostartDisableMock).toHaveBeenCalledTimes(1);
  });
});

describe('api event subscriptions', () => {
  it('onCloseRequested registers "close-requested" and returns a working unsubscribe', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);
    const callback = vi.fn();

    const unsubscribe = api.onCloseRequested(callback);
    expect(listenMock).toHaveBeenCalledExactlyOnceWith('close-requested', expect.any(Function));

    // Invoke the handler passed to `listen` to confirm it forwards to the callback.
    const handler = listenMock.mock.calls[0][1] as () => void;
    handler();
    expect(callback).toHaveBeenCalledTimes(1);

    unsubscribe();
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it('onNotesChanged registers "notes-changed" and unsubscribes via the returned fn', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);
    const callback = vi.fn();

    const unsubscribe = api.onNotesChanged(callback);
    expect(listenMock).toHaveBeenCalledExactlyOnceWith('notes-changed', expect.any(Function));
    (listenMock.mock.calls[0][1] as () => void)();
    expect(callback).toHaveBeenCalledTimes(1);

    unsubscribe();
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it('onContextChanged registers "context-changed" and unsubscribes via the returned fn', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);
    const callback = vi.fn();

    const unsubscribe = api.onContextChanged(callback);
    expect(listenMock).toHaveBeenCalledExactlyOnceWith('context-changed', expect.any(Function));
    (listenMock.mock.calls[0][1] as () => void)();
    expect(callback).toHaveBeenCalledTimes(1);

    unsubscribe();
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it('onAuthCallback registers "auth-callback" and forwards the event payload', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);
    const callback = vi.fn();

    const unsubscribe = api.onAuthCallback(callback);
    expect(listenMock).toHaveBeenCalledExactlyOnceWith('auth-callback', expect.any(Function));
    const handler = listenMock.mock.calls[0][1] as (e: { payload: string }) => void;
    handler({ payload: 'notefix://auth?code=1' });
    expect(callback).toHaveBeenCalledExactlyOnceWith('notefix://auth?code=1');

    unsubscribe();
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it('onSyncStatus registers "sync-status" and forwards the event payload', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);
    const callback = vi.fn();
    const status = { state: 'synced' as const, lastSyncedAt: 1, pending: 0 };

    const unsubscribe = api.onSyncStatus(callback);
    expect(listenMock).toHaveBeenCalledExactlyOnceWith('sync-status', expect.any(Function));
    const handler = listenMock.mock.calls[0][1] as (e: { payload: typeof status }) => void;
    handler({ payload: status });
    expect(callback).toHaveBeenCalledExactlyOnceWith(status);

    unsubscribe();
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it('onTrayEvent registers all three tray event names and routes each to its handler', async () => {
    const unlistens = [vi.fn(), vi.fn(), vi.fn()];
    listenMock
      .mockResolvedValueOnce(unlistens[0])
      .mockResolvedValueOnce(unlistens[1])
      .mockResolvedValueOnce(unlistens[2]);

    const handlers = { newNote: vi.fn(), openNote: vi.fn(), openSettings: vi.fn() };
    const unsubscribe = api.onTrayEvent(handlers);

    expect(listenMock).toHaveBeenNthCalledWith(1, 'tray://new-note', expect.any(Function));
    expect(listenMock).toHaveBeenNthCalledWith(2, 'tray://open-note', expect.any(Function));
    expect(listenMock).toHaveBeenNthCalledWith(3, 'tray://open-settings', expect.any(Function));

    (listenMock.mock.calls[0][1] as () => void)();
    expect(handlers.newNote).toHaveBeenCalledTimes(1);

    (listenMock.mock.calls[1][1] as (e: { payload: string }) => void)({ payload: 'n1' });
    expect(handlers.openNote).toHaveBeenCalledExactlyOnceWith('n1');

    (listenMock.mock.calls[2][1] as () => void)();
    expect(handlers.openSettings).toHaveBeenCalledTimes(1);

    unsubscribe();
    await vi.waitFor(() => unlistens.forEach(u => expect(u).toHaveBeenCalledTimes(1)));
  });
});

describe('api window wrappers', () => {
  it('setWindowTitle(title) sets the current window title', async () => {
    await api.setWindowTitle('My Note');
    expect(setTitleMock).toHaveBeenCalledExactlyOnceWith('My Note');
  });

  it('toggleAlwaysOnTop(current) flips the state and resolves to the new state', async () => {
    const result = await api.toggleAlwaysOnTop(false);
    expect(setAlwaysOnTopMock).toHaveBeenCalledExactlyOnceWith(true);
    expect(result).toBe(true);
  });

  it('toggleAlwaysOnTop(true) flips back to false', async () => {
    const result = await api.toggleAlwaysOnTop(true);
    expect(setAlwaysOnTopMock).toHaveBeenCalledExactlyOnceWith(false);
    expect(result).toBe(false);
  });

  it('closeWindow() closes the current window', async () => {
    await api.closeWindow();
    expect(closeMock).toHaveBeenCalledTimes(1);
  });

  it('startResize() starts a SouthEast resize drag', async () => {
    await api.startResize();
    expect(startResizeDraggingMock).toHaveBeenCalledExactlyOnceWith('SouthEast');
  });

  it('windowProbe() resolves true when title()+setTitle() both succeed', async () => {
    titleMock.mockResolvedValueOnce('Notefix');
    const result = await api.windowProbe();
    expect(titleMock).toHaveBeenCalledTimes(1);
    expect(setTitleMock).toHaveBeenCalledExactlyOnceWith('Notefix');
    expect(result).toBe(true);
  });

  it('windowProbe() resolves false when the window call rejects', async () => {
    titleMock.mockRejectedValueOnce(new Error('no window'));
    const result = await api.windowProbe();
    expect(result).toBe(false);
  });
});

describe('api.getAppInfo / openExternal', () => {
  it('getAppInfo() returns the static name/description plus the live app version', async () => {
    getVersionMock.mockResolvedValueOnce('0.7.0');
    const info = await api.getAppInfo();
    expect(info).toEqual({ name: 'Notefix', version: '0.7.0', description: 'Simple better note app' });
  });

  it('openExternal(url) delegates to the opener plugin', async () => {
    await api.openExternal('https://example.com');
    expect(openUrlMock).toHaveBeenCalledExactlyOnceWith('https://example.com');
  });
});
