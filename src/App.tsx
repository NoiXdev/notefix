import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from './api';
import { useNotes } from './hooks/useNotes';
import { useFolders } from './hooks/useFolders';
import { useSettings } from './hooks/useSettings';
import NoteList from './components/NoteList';
import NoteEditor from './components/NoteEditor';
import Logo from './components/Logo';
import Settings from './components/Settings';
import DeleteFolderModal from './components/DeleteFolderModal';
import CloseDialog from './components/CloseDialog';
import ExportDialog from './components/ExportDialog';
import ExportFormatModal from './components/ExportFormatModal';
import Dashboard from './components/Dashboard';
import SystemCheckModal from './components/SystemCheckModal';
import { runSystemChecks, type SystemCheck } from './systemChecks';
import { exportBase64, exportBundle } from './export';
import { exportNote, type ExportFormat } from './export/exporters';
import { resolveBindings, eventToCombo } from './shortcuts';
import i18n from './i18n';
import { resolveLang } from './i18n/lang';
import type { Folder, Stats } from './types';

const windowNoteId = new URLSearchParams(window.location.search).get('windowNoteId');

export default function App() {
  const { t } = useTranslation();
  const { notes, loading, createNote, updateNote, deleteNote, setPinned, setArchived, setColor, setDue, setFolder, reorderNotes, trashed, restoreNote, purgeNote, emptyTrash } = useNotes();
  const { folders, createFolder, renameFolder, deleteFolder, reorderFolders, setFolderIcon, setFolderColor, setFolderSort } = useFolders();
  const { settings, setSetting, loaded } = useSettings();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [folderToDelete, setFolderToDelete] = useState<Folder | null>(null);
  const [closePrompt, setClosePrompt] = useState(false);
  const [exportReq, setExportReq] = useState<{ ids: string[]; name: string } | null>(null);
  const requestExport = (ids: string[], name: string) => setExportReq({ ids, name });
  const [exportNoteState, setExportNoteState] = useState<import('./types').Note | null>(null);
  const [view, setView] = useState<'editor' | 'dashboard'>('editor');
  const [dashEdit, setDashEdit] = useState(false);
  const [stats, setStats] = useState<Stats | null>(null);
  const [sysProblems, setSysProblems] = useState<SystemCheck[] | null>(null);
  const [settingsPage, setSettingsPage] = useState<'diagnostics' | undefined>(undefined);
  const initView = useRef(false);
  const selectNote = (id: string) => { setSelectedId(id); setView('editor'); };

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

  useEffect(() => { void i18n.changeLanguage(resolveLang(settings.language, navigator.language)); }, [settings.language]);

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

  const handleCreate = async () => {
    const id = await createNote();
    setSelectedId(id);
    setView('editor');
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
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
      if (combo === bindings.newFolder) { e.preventDefault(); void createFolder(i18n.t('noteList.newFolderName'), null); return; }
      if (combo === bindings.newNote) { e.preventDefault(); void handleCreate(); return; }
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
  }, [notes, selectedId, selectedNote, showSettings, createFolder, handleCreate, setArchived, settings.shortcuts]);

  if (windowNoteId) {
    if (loading) {
      return (
        <div className="flex h-screen items-center justify-center" style={{ background: '#fef9c3' }} />
      );
    }
    const note = notes.find(n => n.id === windowNoteId);
    return note
      ? <div className="h-screen"><NoteEditor note={note} onChange={updateNote} isWindow onSetDue={setDue} autosaveDelay={settings.autosaveDelay} /></div>
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

  return (
    <>
      {showSettings && (
        <Settings onClose={() => setShowSettings(false)} settings={settings} onSetSetting={setSetting} onExport={requestExport} initialPage={settingsPage} />
      )}
      {!showSettings && (
      <div className="flex h-screen overflow-hidden">
      <NoteList
        notes={notes}
        folders={folders}
        selectedId={selectedId}
        onSelect={selectNote}
        onCreate={handleCreate}
        onDelete={handleDelete}
        onOpenSettings={() => { setSettingsPage(undefined); setShowSettings(true); }}
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
      />
      <main className="flex-1 overflow-hidden">
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
          <NoteEditor note={selectedNote} onChange={updateNote} onSetDue={setDue} autosaveDelay={settings.autosaveDelay} />
        ) : (
          <div className="flex h-full items-center justify-center" style={{ background: '#fef9c3' }}>
            <div className="text-center" style={{ color: '#b59f3b' }}>
              <Logo size={64} className="mx-auto mb-3 opacity-40" />
              <p className="text-sm">{t('common.selectOrCreate')}</p>
            </div>
          </div>
        )}
      </main>
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
    </>
  );
}
