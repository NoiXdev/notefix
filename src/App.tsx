import { useState, useEffect, useRef } from 'react';
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
import Dashboard from './components/Dashboard';
import type { Folder, Stats } from './types';

const windowNoteId = new URLSearchParams(window.location.search).get('windowNoteId');

export default function App() {
  const { notes, loading, createNote, updateNote, deleteNote, setPinned, setArchived, setColor, setDue, setFolder, reorderNotes, trashed, restoreNote, purgeNote, emptyTrash } = useNotes();
  const { folders, createFolder, renameFolder, deleteFolder, reorderFolders, setFolderIcon, setFolderColor, setFolderSort } = useFolders();
  const { settings, setSetting, loaded } = useSettings();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [folderToDelete, setFolderToDelete] = useState<Folder | null>(null);
  const [closePrompt, setClosePrompt] = useState(false);
  const [view, setView] = useState<'editor' | 'dashboard'>('editor');
  const [dashEdit, setDashEdit] = useState(false);
  const [stats, setStats] = useState<Stats | null>(null);
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

  useEffect(() => { api.stats().then(setStats); }, [notes]);
  useEffect(() => api.onCloseRequested(() => setClosePrompt(true)), []);

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
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (showSettings) return;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.shiftKey && e.key.toLowerCase() === 'n') { e.preventDefault(); void createFolder('Neuer Ordner', null); return; }
      if (mod && e.key.toLowerCase() === 'n') { e.preventDefault(); void handleCreate(); return; }
      if (mod && e.key.toLowerCase() === 'e' && selectedNote) { e.preventDefault(); setArchived(selectedNote.id, !selectedNote.archived); return; }
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        const list = notes.filter(n => !n.archived && !n.deletedAt);
        if (!list.length) return;
        e.preventDefault();
        const idx = list.findIndex(n => n.id === selectedId);
        const next = idx === -1 ? list[0] : list[e.key === 'ArrowDown' ? Math.min(list.length - 1, idx + 1) : Math.max(0, idx - 1)];
        if (next) selectNote(next.id);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [notes, selectedId, selectedNote, showSettings, createFolder, handleCreate, setArchived]);

  if (windowNoteId) {
    if (loading) {
      return (
        <div className="flex h-screen items-center justify-center" style={{ background: '#fef9c3' }} />
      );
    }
    const note = notes.find(n => n.id === windowNoteId);
    return note
      ? <div className="h-screen"><NoteEditor note={note} onChange={updateNote} isWindow onSetDue={setDue} autosaveDelay={settings.autosaveDelay} /></div>
      : <div className="flex h-screen items-center justify-center text-gray-400 text-sm">Note not found.</div>;
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

  if (showSettings) {
    return <Settings onClose={() => setShowSettings(false)} settings={settings} onSetSetting={setSetting} />;
  }

  return (
    <div className="flex h-screen overflow-hidden">
      <NoteList
        notes={notes}
        folders={folders}
        selectedId={selectedId}
        onSelect={selectNote}
        onCreate={handleCreate}
        onDelete={handleDelete}
        onOpenSettings={() => setShowSettings(true)}
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
              <p className="text-sm">Select a note or create a new one</p>
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
      {closePrompt && (
        <CloseDialog
          onMinimize={remember => { if (remember) setSetting('closeAction', 'minimize'); api.hideMain(); setClosePrompt(false); }}
          onQuit={remember => { if (remember) setSetting('closeAction', 'quit'); api.quitApp(); }}
          onCancel={() => setClosePrompt(false)}
        />
      )}
    </div>
  );
}
