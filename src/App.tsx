import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faLock } from '@fortawesome/free-solid-svg-icons';
import { api } from './api';
import { useNotes } from './hooks/useNotes';
import { useFolders } from './hooks/useFolders';
import { useSettings } from './hooks/useSettings';
import { useIsMobile } from './hooks/useIsMobile';
import { useVault } from './hooks/useVault';
import NoteList from './components/NoteList';
import CombinedNoteList from './components/CombinedNoteList';
import NoteEditor from './components/NoteEditor';
import Logo from './components/Logo';
import Settings, { type Page as SettingsPage } from './components/Settings';
import DeleteFolderModal from './components/DeleteFolderModal';
import CloseDialog from './components/CloseDialog';
import ExportDialog from './components/ExportDialog';
import ExportFormatModal from './components/ExportFormatModal';
import Dashboard from './components/Dashboard';
import SystemCheckModal from './components/SystemCheckModal';
import WorkspacePicker from './components/WorkspacePicker';
import UpdateBanner from './components/UpdateBanner';
import SearchModal from './components/SearchModal';
import ConfettiEasterEgg from './components/ConfettiEasterEgg';
import VaultSetup from './components/VaultSetup';
import VaultUnlock from './components/VaultUnlock';
import ConfirmDialog from './components/ConfirmDialog';
import WhatsNew from './components/WhatsNew';
import { shouldShowUpdateBanner } from './updateCheck';
import type { UpdateInfo, ReleaseInfo } from './api';
import { releasesSince, isNewer } from './version';
import { runSystemChecks, type SystemCheck } from './systemChecks';
import { exportBase64, exportBundle } from './export';
import { exportNote, type ExportFormat } from './export/exporters';
import { resolveBindings, eventToCombo, OPEN_CONTEXTS_EVENT } from './shortcuts';
import { nextContextId, type ContextInfo } from './contexts';
import i18n from './i18n';
import { resolveLang } from './i18n/lang';
import type { Folder, Stats } from './types';

const windowNoteId = new URLSearchParams(window.location.search).get('windowNoteId');

/**
 * Remembers that the "images stay unencrypted" hint has been acknowledged, so
 * it is shown once per device rather than before every protected note.
 * Storage can throw (private mode, disabled site data) — a hint that cannot be
 * remembered is simply shown again, never an error.
 */
const IMAGES_HINT_KEY = 'vault.imagesHintSeen';
const imagesHintSeen = () => {
  try {
    return localStorage.getItem(IMAGES_HINT_KEY) === '1';
  } catch {
    return false;
  }
};
const rememberImagesHint = () => {
  try {
    localStorage.setItem(IMAGES_HINT_KEY, '1');
  } catch {
    /* nothing to remember it in — the hint just shows again */
  }
};

export default function App() {
  const { t } = useTranslation();
  const { notes, loading, createNote, updateNote, deleteNote, setPinned, setArchived, setColor, setDue, setFolder, reorderNotes, trashed, restoreNote, purgeNote, emptyTrash, reload: reloadNotes } = useNotes();
  const { folders, createFolder, renameFolder, deleteFolder, reorderFolders, setFolderIcon, setFolderColor, setFolderSort, reload: reloadFolders } = useFolders();
  const { settings, setSetting, loaded, reload: reloadSettings } = useSettings();
  const vault = useVault();
  const [pendingProtect, setPendingProtect] = useState<{ kind: 'note' | 'folder'; id: string; next: boolean } | null>(null);
  const [vaultDialog, setVaultDialog] = useState<'setup' | 'unlock' | null>(null);
  // A protect that is waiting on the one-time "images stay unencrypted" hint.
  const [imagesHint, setImagesHint] = useState<{ kind: 'note' | 'folder'; id: string; next: boolean } | null>(null);
  // 'perNote' lock scope: notes unlocked this session, so re-locking the note
  // list doesn't force re-entering the passphrase for a note already shown.
  const [revealedNotes, setRevealedNotes] = useState<Set<string>>(new Set());
  const [pendingReveal, setPendingReveal] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [folderToDelete, setFolderToDelete] = useState<Folder | null>(null);
  const [closePrompt, setClosePrompt] = useState(false);
  const [exportReq, setExportReq] = useState<{ ids: string[]; name: string } | null>(null);
  const requestExport = (ids: string[], name: string) => setExportReq({ ids, name });
  const [exportNoteState, setExportNoteState] = useState<import('./types').NoteMeta | null>(null);
  const [view, setView] = useState<'editor' | 'dashboard'>('editor');
  const [dashEdit, setDashEdit] = useState(false);
  const [stats, setStats] = useState<Stats | null>(null);
  const [sysProblems, setSysProblems] = useState<SystemCheck[] | null>(null);
  const [settingsPage, setSettingsPage] = useState<SettingsPage | undefined>(undefined);
  const [bindCtx, setBindCtx] = useState<string | null>(null);
  const [activeContextId, setActiveContextId] = useState<string>('');
  const [contexts, setContexts] = useState<ContextInfo[]>([]);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [whatsNewAuto, setWhatsNewAuto] = useState<{ releases: ReleaseInfo[]; current: string } | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  // Mobile: single-column. `mobileEditor` = showing the editor (vs. the list).
  const isMobile = useIsMobile();
  const [mobileEditor, setMobileEditor] = useState(false);
  const pendingSelectRef = useRef<string | null>(null);
  const initView = useRef(false);
  const selectNote = (id: string) => { setSelectedId(id); setView('editor'); setMobileEditor(true); };
  const selectCombined = (noteId: string, contextId: string) => {
    setMobileEditor(true);
    if (contextId !== activeContextId) {
      pendingSelectRef.current = noteId;
      void api.contexts.switch(contextId);
    } else {
      selectNote(noteId);
    }
  };

  // Auto-select the first note on load
  useEffect(() => {
    if (!selectedId && notes.length > 0) {
      setSelectedId(notes[0].id);
    }
  }, [notes, selectedId]);

  useEffect(() => {
    if (loaded && !initView.current) {
      initView.current = true;
      if (settings.startView === 'dashboard') setView('dashboard');
    }
  }, [loaded, settings.startView]);

  const checkedRef = useRef(false);
  useEffect(() => {
    if (!loaded || checkedRef.current) return;
    checkedRef.current = true;
    void runSystemChecks(settings).then(checks => {
      const problems = checks.filter(c => c.status === 'error');
      if (problems.length) setSysProblems(problems);
    });
  }, [loaded, settings]);

  useEffect(() => { api.stats().then(setStats); }, [notes]);
  useEffect(() => api.onCloseRequested(() => setClosePrompt(true)), []);

  useEffect(() => {
    return api.onContextChanged(() => {
      if (pendingSelectRef.current) {
        setSelectedId(pendingSelectRef.current);
        pendingSelectRef.current = null;
      } else {
        setSelectedId(null);
      }
      setView('editor');
      void reloadNotes();
      void reloadFolders();
      void reloadSettings();
      // Switching contexts locks the vault backend-side (the DEK belongs to the
      // previous context's DB); refresh so the UI reflects the locked state.
      void vault.refresh();
    });
  }, [reloadNotes, reloadFolders, reloadSettings, vault.refresh]);

  // Prompt to bind a workspace when the active context is an unbound server context.
  useEffect(() => {
    const check = () => void api.contexts.list().then(cs => {
      setContexts(cs);
      const active = cs.find(c => c.active);
      setActiveContextId(active?.id ?? '');
      if (active?.kind === 'server' && !active.workspaceId) setBindCtx(active.id);
    });
    check(); // initial (e.g. app starts on an unbound server context)
    return api.onContextChanged(check);
  }, []);

  // Complete add-server flows centrally: the browser redirect (notefix://auth)
  // arrives as an auth-callback event regardless of which UI started the flow.
  // A successful exchange emits context-changed, which reloads everything.
  useEffect(() => api.onAuthCallback((url) => {
    void api.contexts.serverAuthComplete(url).catch(() => {});
  }), []);

  useEffect(() => { void i18n.changeLanguage(resolveLang(settings.language, navigator.language)); }, [settings.language]);

  // Apply the color theme: 'butter' is the :root default, others set data-theme.
  useEffect(() => {
    const root = document.documentElement;
    if (settings.theme === 'butter') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', settings.theme);
  }, [settings.theme]);

  // Editor typography + width (drive .ProseMirror via CSS vars).
  useEffect(() => {
    const s = document.documentElement.style;
    s.setProperty('--editor-line-height', { normal: '1.6', relaxed: '1.9', loose: '2.2' }[settings.editorLineHeight]);
    s.setProperty('--editor-font-size', { small: '14px', medium: '16px', large: '18px', xlarge: '20px' }[settings.editorFontSize]);
    s.setProperty('--editor-font-family', {
      sans: 'ui-sans-serif, system-ui, -apple-system, sans-serif',
      serif: 'ui-serif, Georgia, Cambria, "Times New Roman", serif',
      mono: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
      rounded: 'ui-rounded, "SF Pro Rounded", system-ui, sans-serif',
    }[settings.editorFontFamily]);
    s.setProperty('--editor-max-width', { full: 'none', medium: '52rem', narrow: '40rem' }[settings.editorWidth]);
  }, [settings.editorLineHeight, settings.editorFontSize, settings.editorFontFamily, settings.editorWidth]);

  // On launch: one silent GitHub-release check (opt-out via checkUpdatesOnStart).
  const updateCheckedRef = useRef(false);
  useEffect(() => {
    if (!loaded || updateCheckedRef.current) return;
    updateCheckedRef.current = true;
    if (!settings.checkUpdatesOnStart) return;
    // Never let the update check break the app (network error, missing backend).
    try {
      void api.checkForUpdate().then(setUpdateInfo).catch(() => {});
    } catch { /* ignore */ }
  }, [loaded, settings.checkUpdatesOnStart]);

  // On launch: cumulative "What's New" changelog since the version the user
  // last saw. Unlike checkForUpdate above, this is a plain network fetch +
  // dialog — no desktop-only Tauri APIs — so it runs on mobile too.
  const whatsNewCheckedRef = useRef(false);
  useEffect(() => {
    if (!loaded || whatsNewCheckedRef.current) return;
    whatsNewCheckedRef.current = true;
    void (async () => {
      const { version: current } = await api.getAppInfo();
      if (settings.lastSeenVersion === '') {
        // Fresh install (or first run after this feature shipped): nothing
        // was actually "missed" — just start tracking from here, silently.
        void setSetting('lastSeenVersion', current);
        return;
      }
      if (!settings.whatsNewOnUpdate || !isNewer(current, settings.lastSeenVersion)) return;
      try {
        const releases = await api.githubReleases();
        const since = releasesSince(releases, settings.lastSeenVersion, current);
        if (since.length > 0) setWhatsNewAuto({ releases: since, current });
        else void setSetting('lastSeenVersion', current);
      } catch {
        // Don't nag on a failed fetch — just record the version and move on.
        void setSetting('lastSeenVersion', current);
      }
    })();
  }, [loaded, settings.lastSeenVersion, settings.whatsNewOnUpdate, setSetting]);

  const mcpAppliedRef = useRef(false);
  useEffect(() => {
    if (!loaded) return;
    // The backend already autostarts the MCP server from persisted settings at
    // launch (see lib.rs setup). Skip this first post-load apply so we don't
    // stop the running server and immediately rebind — that restart races the
    // not-yet-released port (EADDRINUSE) and can leave the server down. Only
    // user-driven config changes (ref already set) trigger a real re-apply.
    if (!mcpAppliedRef.current) { mcpAppliedRef.current = true; return; }
    void api.mcpApplyConfig({
      enabled: settings.mcpEnabled,
      bind: settings.mcpBind,
      port: settings.mcpPort,
      token: settings.mcpToken,
      authRequired: settings.mcpAuthRequired,
      allowWrite: settings.mcpAllowWrite,
    });
  }, [loaded, settings.mcpEnabled, settings.mcpBind, settings.mcpPort, settings.mcpToken, settings.mcpAuthRequired, settings.mcpAllowWrite]);

  useEffect(() => {
    return api.onTrayEvent({
      newNote: async () => {
        const id = await createNote();
        selectNote(id);
      },
      openNote: (id: string) => { setShowSettings(false); selectNote(id); },
      openSettings: () => setShowSettings(true),
    });
  }, [createNote]);

  const selectedNote = notes.find(n => n.id === selectedId) ?? null;

  const handleCreate = async (): Promise<string> => {
    const id = await createNote();
    setSelectedId(id);
    setView('editor');
    return id;
  };

  useEffect(() => {
    const bindings = resolveBindings(settings.shortcuts);
    const onKey = (e: KeyboardEvent) => {
      const combo = eventToCombo(e);
      if (!combo) return;
      if (windowNoteId) {
        if (combo === bindings.closeWindow) { e.preventDefault(); void api.closeWindow(); }
        return;
      }
      if (showSettings) return;
      // Note finder works even while editing (before the input/editable guard).
      if (combo === bindings.openSearch) { e.preventDefault(); setSearchOpen(o => !o); return; }
      // Locking the vault is a security action — it should work even while
      // editing, same as search above.
      if (combo === bindings.lockVault) { e.preventDefault(); if (vault.status.exists && vault.status.unlocked) void vault.lock(); return; }
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
      if (combo === bindings.newFolder) { e.preventDefault(); void createFolder(i18n.t('noteList.newFolderName'), null); return; }
      if (combo === bindings.newNote) { e.preventDefault(); void handleCreate(); return; }
      if (combo === bindings.switchContextNext) {
        e.preventDefault();
        const nextId = nextContextId(contexts);
        if (nextId) void api.contexts.switch(nextId);
        return;
      }
      if (combo === bindings.openContextPicker) {
        e.preventDefault();
        window.dispatchEvent(new Event(OPEN_CONTEXTS_EVENT));
        return;
      }
      if (combo === bindings.archive && selectedNote) { e.preventDefault(); setArchived(selectedNote.id, !selectedNote.archived); return; }
      if (combo === bindings.navPrev || combo === bindings.navNext) {
        const list = notes.filter(n => !n.archived && !n.deletedAt);
        if (!list.length) return;
        e.preventDefault();
        const idx = list.findIndex(n => n.id === selectedId);
        const dir = combo === bindings.navNext ? 1 : -1;
        const next = idx === -1 ? list[0] : list[dir === 1 ? Math.min(list.length - 1, idx + 1) : Math.max(0, idx - 1)];
        if (next) selectNote(next.id);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [notes, selectedId, selectedNote, showSettings, createFolder, handleCreate, setArchived, settings.shortcuts, contexts, vault.status.exists, vault.status.unlocked, vault.lock]);

  // 'perNote' lock scope: whenever the vault locks (any path — idle timer,
  // hide/sleep, the shortcut, the sidebar button, Security page), forget which
  // notes were revealed this session so they require unlocking again.
  useEffect(() => {
    if (!vault.status.unlocked) setRevealedNotes(new Set());
  }, [vault.status.unlocked]);

  // Auto-lock: idle timer. Only while the vault is actually unlocked and the
  // user opted into locking after N minutes; any user activity resets the
  // clock. Skipped in the detached note window (windowNoteId), which has no
  // vault UI.
  useEffect(() => {
    if (windowNoteId) return;
    if (!vault.status.unlocked || !settings.autoLockIdle) return;
    const timeoutMs = settings.autoLockMinutes * 60000;
    let timer: ReturnType<typeof setTimeout>;
    const scheduleLock = () => {
      clearTimeout(timer);
      timer = setTimeout(() => { void vault.lock(); }, timeoutMs);
    };
    window.addEventListener('mousemove', scheduleLock);
    window.addEventListener('keydown', scheduleLock);
    window.addEventListener('mousedown', scheduleLock);
    window.addEventListener('touchstart', scheduleLock);
    scheduleLock();
    return () => {
      clearTimeout(timer);
      window.removeEventListener('mousemove', scheduleLock);
      window.removeEventListener('keydown', scheduleLock);
      window.removeEventListener('mousedown', scheduleLock);
      window.removeEventListener('touchstart', scheduleLock);
    };
  }, [vault.status.unlocked, settings.autoLockIdle, settings.autoLockMinutes, vault.lock]);

  // Auto-lock: hide/sleep. autoLockOnHide locks whenever the app is
  // backgrounded; autoLockOnSleep additionally covers system sleep/lock, for
  // which there is no dedicated Tauri event — document visibility is the
  // practical proxy for both cases (the OS hides/suspends the webview in
  // either scenario).
  useEffect(() => {
    if (windowNoteId) return;
    if (!vault.status.unlocked) return;
    if (!settings.autoLockOnHide && !settings.autoLockOnSleep) return;
    const onVisibility = () => { if (document.hidden) void vault.lock(); };
    document.addEventListener('visibilitychange', onVisibility);
    return () => document.removeEventListener('visibilitychange', onVisibility);
  }, [vault.status.unlocked, settings.autoLockOnHide, settings.autoLockOnSleep, vault.lock]);

  if (windowNoteId) {
    if (loading) {
      return (
        <div className="flex h-screen items-center justify-center" style={{ background: 'var(--paper)' }} />
      );
    }
    const note = notes.find(n => n.id === windowNoteId);
    const noteLocked = note?.protected && (!vault.status.unlocked || (settings.vaultLockScope === 'perNote' && !revealedNotes.has(note.id)));
    if (note && noteLocked) {
      return (
        <div className="flex h-screen items-center justify-center" style={{ background: 'var(--paper)' }}>
          <div className="text-center" style={{ color: 'var(--ink-muted)' }}>
            <FontAwesomeIcon icon={faLock} className="text-3xl mb-3 opacity-40" />
            <p className="text-sm">{t('vault.noteLocked')}</p>
          </div>
        </div>
      );
    }
    return note
      ? <div className="h-screen"><NoteEditor note={note} onChange={updateNote} isWindow onSetDue={setDue} autosaveDelay={settings.autosaveDelay} linkPreviewEnabled={settings.linkPreviewEnabled} linkPreviewMode={settings.linkPreviewMode} copyFormat={settings.copyFormat} countShow={settings.editorCountShow} countPos={settings.editorCountPos} invisibles={settings.editorInvisibles} toolbarPos={settings.editorToolbarPos} /></div>
      : <div className="flex h-screen items-center justify-center text-gray-400 text-sm">{t('common.noteNotFound')}</div>;
  }

  const handleDelete = (id: string) => {
    if (selectedId === id) {
      const remaining = notes.filter(n => n.id !== id);
      setSelectedId(remaining[0]?.id ?? null);
    }
    deleteNote(id);
  };

  const countInSubtree = (folderId: string) => {
    const subIds = new Set<string>([folderId]);
    let changed = true;
    while (changed) {
      changed = false;
      for (const f of folders) if (f.parentId && subIds.has(f.parentId) && !subIds.has(f.id)) { subIds.add(f.id); changed = true; }
    }
    const noteCount = notes.filter(n => n.folderId && subIds.has(n.folderId)).length;
    const subfolderCount = subIds.size - 1;
    return { noteCount, subfolderCount };
  };

  const requestDeleteFolder = (folder: Folder) => {
    const { noteCount, subfolderCount } = countInSubtree(folder.id);
    if (noteCount === 0 && subfolderCount === 0) deleteFolder(folder.id, 'reparent');
    else setFolderToDelete(folder);
  };

  // Apply a protect/lock toggle once the vault is confirmed unlocked, then
  // refresh — the same explicit-reload pattern useFolders/useNotes use after
  // a mutation, since the change-broadcast skips the sender window.
  const applyProtect = async (kind: 'note' | 'folder', id: string, next: boolean) => {
    if (kind === 'note') await api.vault.protectNote(id, next);
    else await api.vault.lockFolder(id, next);
    await reloadNotes();
    await reloadFolders();
  };

  // Route a protect through the vault gate: set up, unlock, or just apply it.
  const gateProtect = (kind: 'note' | 'folder', id: string, next: boolean) => {
    if (!vault.status.exists) {
      setPendingProtect({ kind, id, next });
      setVaultDialog('setup');
    } else if (!vault.status.unlocked) {
      setPendingProtect({ kind, id, next });
      setVaultDialog('unlock');
    } else {
      void applyProtect(kind, id, next);
    }
  };

  /** Whether this note's stored HTML embeds an image. */
  const noteHasImages = async (id: string) => {
    try {
      return (await api.notes.loadOne(id)).includes('<img');
    } catch {
      return false;
    }
  };

  const requestProtect = (kind: 'note' | 'folder', id: string, next: boolean) => {
    // Protecting a note seals its HTML, but the images it references stay as
    // plain files on disk. Say so once, before the first such note is locked.
    if (kind === 'note' && next && !imagesHintSeen()) {
      void noteHasImages(id).then(has => {
        if (has) setImagesHint({ kind, id, next });
        else gateProtect(kind, id, next);
      });
      return;
    }
    gateProtect(kind, id, next);
  };

  // "Hide from MCP" is a plaintext local flag — no vault involved, so (unlike
  // protect/lock) it applies directly without an unlock gate.
  const setNoteMcpHidden = async (id: string, next: boolean) => {
    await api.notes.setMcpHidden(id, next);
    await reloadNotes();
  };
  const setFolderMcpHidden = async (id: string, next: boolean) => {
    await api.folders.setMcpHidden(id, next);
    await reloadFolders();
  };

  const cancelVaultDialog = () => { setVaultDialog(null); setPendingProtect(null); };

  const afterUnlockOrSetup = () => {
    setVaultDialog(null);
    if (pendingReveal) {
      const id = pendingReveal;
      setPendingReveal(null);
      setRevealedNotes(prev => new Set(prev).add(id));
    }
    if (pendingProtect) {
      const p = pendingProtect;
      setPendingProtect(null);
      void applyProtect(p.kind, p.id, p.next);
    }
  };

  return (
    <>
      <ConfettiEasterEgg />
      {searchOpen && (
        <SearchModal
          scope={settings.searchScope}
          onScope={s => setSetting('searchScope', s)}
          onClose={() => setSearchOpen(false)}
          onOpenNote={(id, contextId) => {
            setShowSettings(false);
            if (contextId) selectCombined(id, contextId); else selectNote(id);
          }}
        />
      )}
      {showSettings && (
        <Settings onClose={() => setShowSettings(false)} settings={settings} onSetSetting={setSetting} onExport={requestExport} initialPage={settingsPage} />
      )}
      {!showSettings && (
      <div className="flex flex-col h-screen overflow-hidden">
      {shouldShowUpdateBanner(updateInfo, settings.updateDismissedVersion) && updateInfo && (
        <UpdateBanner
          info={updateInfo}
          onDownload={() => void api.openExternal(updateInfo.url)}
          onDismiss={() => void setSetting('updateDismissedVersion', updateInfo.latest)}
        />
      )}
      <div className={`flex flex-1 min-h-0 overflow-hidden ${!isMobile && settings.sidebarSide === 'right' ? 'flex-row-reverse' : ''}`}>
      {(!isMobile || !mobileEditor) && (settings.sidebarMode === 'combined' ? (
        <CombinedNoteList
          selectedId={selectedId}
          activeContextId={activeContextId}
          onSelectNote={selectCombined}
          onCreate={handleCreate}
          onOpenSettings={() => { setSettingsPage(undefined); setShowSettings(true); }}
          onOpenSearch={() => setSearchOpen(true)}
          onOpenContexts={() => { setSettingsPage('contexts'); setShowSettings(true); }}
          dateFormat={settings.dateFormat}
          mobile={isMobile}
          side={isMobile ? 'left' : settings.sidebarSide}
          vaultExists={vault.status.exists}
          vaultUnlocked={vault.status.unlocked}
          onLockVault={() => void vault.lock()}
        />
      ) : (
      <NoteList
        mobile={isMobile}
        notes={notes}
        folders={folders}
        selectedId={selectedId}
        onSelect={selectNote}
        onCreate={handleCreate}
        onDelete={handleDelete}
        onOpenSettings={() => { setSettingsPage(undefined); setShowSettings(true); }}
        onOpenContexts={() => { setSettingsPage('contexts'); setShowSettings(true); }}
        onOpenSearch={() => setSearchOpen(true)}
        onOpenDashboard={() => setView('dashboard')}
        onTogglePin={setPinned}
        onArchive={setArchived}
        onSetColor={setColor}
        onMoveNote={setFolder}
        onCreateFolder={createFolder}
        onRenameFolder={renameFolder}
        onDeleteFolder={requestDeleteFolder}
        onReorderNotes={reorderNotes}
        onReorderFolders={reorderFolders}
        onSetFolderIcon={setFolderIcon}
        onSetFolderColor={setFolderColor}
        onSetFolderSort={setFolderSort}
        dateFormat={settings.dateFormat}
        pinnedScope={settings.pinnedScope}
        folderColorStyle={settings.folderColorStyle}
        compactTree={settings.compactTree}
        treeProgress={settings.treeProgress}
        trashed={trashed}
        trashEnabled={settings.trashEnabled}
        onRestore={restoreNote}
        onPurge={purgeNote}
        onEmptyTrash={emptyTrash}
        onExportNote={(n) => setExportNoteState(n)}
        onProtectNote={(id, next) => requestProtect('note', id, next)}
        onLockFolder={(id, next) => requestProtect('folder', id, next)}
        onSetNoteMcpHidden={(id, next) => void setNoteMcpHidden(id, next)}
        onSetFolderMcpHidden={(id, next) => void setFolderMcpHidden(id, next)}
        vaultExists={vault.status.exists}
        vaultUnlocked={vault.status.unlocked}
        onLockVault={() => void vault.lock()}
      />
      ))}
      {(!isMobile || mobileEditor) && (
      <main className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {isMobile && (
          <button
            onClick={() => setMobileEditor(false)}
            className="shrink-0 flex items-center gap-1.5 px-4 pb-3 text-[15px] font-medium border-b"
            style={{ background: 'var(--panel)', borderColor: 'var(--line)', color: 'var(--ink)', paddingTop: 'calc(0.75rem + env(safe-area-inset-top))' }}
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="15 18 9 12 15 6" /></svg>
            {t('common.notesBack')}
          </button>
        )}
        <div className="flex-1 min-h-0 overflow-hidden">
        {view === 'dashboard' ? (
          <Dashboard
            notes={notes}
            folders={folders}
            stats={stats}
            layout={settings.dashboardLayout}
            editMode={dashEdit}
            onSelectNote={selectNote}
            onCreateNote={handleCreate}
            onChangeLayout={l => setSetting('dashboardLayout', l)}
            onToggleEdit={() => setDashEdit(v => !v)}
          />
        ) : selectedNote ? (
          selectedNote.protected && (!vault.status.unlocked || (settings.vaultLockScope === 'perNote' && !revealedNotes.has(selectedNote.id))) ? (
            <div className="flex h-full items-center justify-center" style={{ background: 'var(--paper)' }}>
              <div className="text-center" style={{ color: 'var(--ink-muted)' }}>
                <FontAwesomeIcon icon={faLock} className="text-4xl mb-3 opacity-40" />
                <p className="text-sm mb-4">{t('vault.noteLocked')}</p>
                <button
                  onClick={() => { setPendingReveal(selectedNote.id); setVaultDialog('unlock'); }}
                  className="px-3 py-1.5 rounded text-sm font-medium"
                  style={{ background: 'var(--line)', color: '#1c1917' }}
                >
                  {t('vault.unlock')}
                </button>
              </div>
            </div>
          ) : (
            <NoteEditor note={selectedNote} onChange={updateNote} onSetDue={setDue} autosaveDelay={settings.autosaveDelay} linkPreviewEnabled={settings.linkPreviewEnabled} linkPreviewMode={settings.linkPreviewMode} copyFormat={settings.copyFormat} findShortcut={resolveBindings(settings.shortcuts).findInNote} countShow={settings.editorCountShow} countPos={settings.editorCountPos} invisibles={settings.editorInvisibles} toolbarPos={settings.editorToolbarPos} />
          )
        ) : (
          <div className="flex h-full items-center justify-center" style={{ background: 'var(--paper)' }}>
            <div className="text-center" style={{ color: 'var(--ink-muted)' }}>
              <Logo size={64} className="mx-auto mb-3 opacity-40" />
              <p className="text-sm">{t('common.selectOrCreate')}</p>
            </div>
          </div>
        )}
        </div>
      </main>
      )}
      </div>
      {folderToDelete && (
        <DeleteFolderModal
          folderName={folderToDelete.name}
          noteCount={countInSubtree(folderToDelete.id).noteCount}
          subfolderCount={countInSubtree(folderToDelete.id).subfolderCount}
          onReparent={() => { deleteFolder(folderToDelete.id, 'reparent'); setFolderToDelete(null); }}
          onRecursive={() => { deleteFolder(folderToDelete.id, 'recursive'); setFolderToDelete(null); }}
          onCancel={() => setFolderToDelete(null)}
        />
      )}
      </div>
      )}
      {closePrompt && (
        <CloseDialog
          onMinimize={remember => { if (remember) setSetting('closeAction', 'minimize'); api.hideMain(); setClosePrompt(false); }}
          onQuit={remember => { if (remember) setSetting('closeAction', 'quit'); api.quitApp(); }}
          onCancel={() => setClosePrompt(false)}
        />
      )}
      {exportReq && (
        <ExportDialog
          onBase64={() => { void exportBase64(exportReq.ids, exportReq.name); setExportReq(null); }}
          onBundle={() => { void exportBundle(exportReq.ids); setExportReq(null); }}
          onCancel={() => setExportReq(null)}
        />
      )}
      {exportNoteState && (
        <ExportFormatModal
          onExport={(f: ExportFormat, mdBundle: boolean) => { const n = exportNoteState; setExportNoteState(null); void exportNote(n, f, mdBundle); }}
          onCancel={() => setExportNoteState(null)}
        />
      )}
      {sysProblems && (
        <SystemCheckModal
          problems={sysProblems}
          onOpenSettings={() => { setSysProblems(null); setSettingsPage('diagnostics'); setShowSettings(true); }}
          onClose={() => setSysProblems(null)}
        />
      )}
      {bindCtx && <WorkspacePicker contextId={bindCtx} onClose={() => setBindCtx(null)} />}
      {whatsNewAuto && (
        <WhatsNew
          releases={whatsNewAuto.releases}
          onClose={() => { void setSetting('lastSeenVersion', whatsNewAuto.current); setWhatsNewAuto(null); }}
        />
      )}
      {imagesHint && (
        <ConfirmDialog
          title={t('vault.imagesUnencryptedTitle')}
          message={t('vault.imagesUnencryptedHint')}
          confirmLabel={t('vault.lockNote')}
          onConfirm={() => {
            const p = imagesHint;
            rememberImagesHint();
            setImagesHint(null);
            gateProtect(p.kind, p.id, p.next);
          }}
          onCancel={() => setImagesHint(null)}
        />
      )}
      {vaultDialog === 'setup' && <VaultSetup setup={vault.setup} onSuccess={afterUnlockOrSetup} onCancel={cancelVaultDialog} />}
      {vaultDialog === 'unlock' && (
        <VaultUnlock
          biometricAvailable={vault.status.biometric}
          recoveryAvailable={vault.status.recoveryHolder}
          unlock={vault.unlock}
          unlockRecovery={vault.unlockRecovery}
          unlockBiometric={vault.unlockBiometric}
          onSuccess={afterUnlockOrSetup}
          onCancel={cancelVaultDialog}
        />
      )}
    </>
  );
}
