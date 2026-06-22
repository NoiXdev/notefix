import { useState, useEffect } from 'react';
import { api } from './api';
import { useNotes } from './hooks/useNotes';
import { useSettings } from './hooks/useSettings';
import NoteList from './components/NoteList';
import NoteEditor from './components/NoteEditor';
import Settings from './components/Settings';

const windowNoteId = new URLSearchParams(window.location.search).get('windowNoteId');

export default function App() {
  const { notes, loading, createNote, updateNote, deleteNote, setPinned } = useNotes();
  const { settings, setSetting } = useSettings();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);

  // Auto-select the first note on load
  useEffect(() => {
    if (!selectedId && notes.length > 0) {
      setSelectedId(notes[0].id);
    }
  }, [notes, selectedId]);

  useEffect(() => {
    return api.onTrayEvent({
      newNote: async () => {
        const id = await createNote();
        setSelectedId(id);
      },
      openNote: (id: string) => {
        setShowSettings(false);
        setSelectedId(id);
      },
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
      ? <div className="h-screen"><NoteEditor note={note} onChange={updateNote} isWindow /></div>
      : <div className="flex h-screen items-center justify-center text-gray-400 text-sm">Note not found.</div>;
  }

  const selectedNote = notes.find(n => n.id === selectedId) ?? null;

  const handleCreate = async () => {
    const id = await createNote();
    setSelectedId(id);
  };

  const handleDelete = (id: string) => {
    if (selectedId === id) {
      const remaining = notes.filter(n => n.id !== id);
      setSelectedId(remaining[0]?.id ?? null);
    }
    deleteNote(id);
  };

  if (showSettings) {
    return <Settings onClose={() => setShowSettings(false)} settings={settings} onSetSetting={setSetting} />;
  }

  return (
    <div className="flex h-screen overflow-hidden">
      <NoteList
        notes={notes}
        selectedId={selectedId}
        onSelect={setSelectedId}
        onCreate={handleCreate}
        onDelete={handleDelete}
        onOpenSettings={() => setShowSettings(true)}
        displayMode={settings.pinnedDisplayMode}
        onTogglePin={setPinned}
      />
      <main className="flex-1 overflow-hidden">
        {selectedNote ? (
          <NoteEditor note={selectedNote} onChange={updateNote} />
        ) : (
          <div className="flex h-full items-center justify-center" style={{ background: '#fef9c3' }}>
            <div className="text-center" style={{ color: '#b59f3b' }}>
              <svg className="mx-auto mb-3 opacity-40" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                <polyline points="14 2 14 8 20 8" />
              </svg>
              <p className="text-sm">Select a note or create a new one</p>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
