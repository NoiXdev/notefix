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
import Dashboard from './components/Dashboard';
import type { Folder, Stats } from './types';

const windowNoteId = new URLSearchParams(window.location.search).get('windowNoteId');

export default function App() {
  const { notes, loading, createNote, updateNote, deleteNote, setPinned, setArchived, setColor, setDue, setFolder, reorderNotes } = useNotes();
  const { folders, createFolder, renameFolder, deleteFolder, reorderFolders, setFolderIcon, setFolderColor } = useFolders();
  const { settings, setSetting, loaded } = useSettings();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [folderToDelete, setFolderToDelete] = useState<Folder | null>(null);
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

  const selectedNote = notes.find(n => n.id === selectedId) ?? null;

  const handleCreate = async () => {
    const id = await createNote();
    setSelectedId(id);
    setView('editor');
  };

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
        dateFormat={settings.dateFormat}
        pinnedScope={settings.pinnedScope}
        folderColorStyle={settings.folderColorStyle}
      />
      <main className="flex-1 overflow-hidden">
        {view === 'dashboard' ? (
          <Dashboard
            notes={notes}
            stats={stats}
            layout={settings.dashboardLayout}
            editMode={dashEdit}
            onSelectNote={selectNote}
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
    </div>
  );
}
