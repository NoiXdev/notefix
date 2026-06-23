import { useEffect, useRef, useState } from 'react';
import { useEditor, EditorContent, type Editor } from '@tiptap/react';
import type { EditorView } from '@tiptap/pm/view';
import StarterKit from '@tiptap/starter-kit';
import Underline from '@tiptap/extension-underline';
import Placeholder from '@tiptap/extension-placeholder';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import { countTasks } from '../tasks';
import { ResizableImage } from './ResizableImage';
import type { Note } from '../types';
import { api } from '../api';

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

async function insertImageFilesIntoView(view: EditorView, files: File[], pos?: number): Promise<void> {
  const images = files.filter(f => f.type.startsWith('image/'));
  if (!images.length) return;
  for (const file of images) {
    const src = await readFileAsDataUrl(file);
    const node = view.state.schema.nodes.image?.create({ src });
    if (!node) continue;
    const insertAt = typeof pos === 'number' ? pos : view.state.selection.to;
    view.dispatch(view.state.tr.insert(insertAt, node));
  }
}

async function insertImageFilesIntoEditor(editor: Editor, files: File[]): Promise<void> {
  const images = files.filter(f => f.type.startsWith('image/'));
  if (!images.length) return;
  for (const file of images) {
    const src = await readFileAsDataUrl(file);
    editor.chain().focus().setImage({ src }).run();
  }
}

function getTitleFromHtml(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  const first = el.firstElementChild;
  return first?.textContent?.trim() || el.textContent?.trim() || 'New note';
}

interface Props {
  note: Note;
  onChange: (id: string, content: string) => void;
  isWindow?: boolean;
}

interface ToolbarBtnProps {
  onClick: () => void;
  active?: boolean;
  title: string;
  children: React.ReactNode;
}

function ToolbarBtn({ onClick, active, title, children }: ToolbarBtnProps) {
  return (
    <button
      onMouseDown={e => { e.preventDefault(); onClick(); }}
      title={title}
      className={`w-8 h-8 flex items-center justify-center rounded text-sm transition-colors select-none ${
        active
          ? 'bg-yellow-500 text-gray-900'
          : 'text-gray-700 hover:bg-yellow-400'
      }`}
    >
      {children}
    </button>
  );
}

export default function NoteEditor({ note, onChange, isWindow = false }: Props) {
  const [pinned, setPinned] = useState(false);
  const [progress, setProgress] = useState({ done: 0, total: 0 });
  const pendingSave = useRef<ReturnType<typeof setTimeout> | null>(null);
  const skipNextUpdate = useRef(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const editor = useEditor({
    extensions: [
      StarterKit,
      Underline,
      Placeholder.configure({ placeholder: 'Start writing…' }),
      TaskList,
      TaskItem.configure({ nested: true }),
      ResizableImage.configure({ inline: false, allowBase64: true }),
    ],
    content: note.content || '<p></p>',
    onUpdate: ({ editor: e }) => {
      if (skipNextUpdate.current) {
        skipNextUpdate.current = false;
        return;
      }
      const html = e.getHTML();
        setProgress(countTasks(html));
      if (isWindow) api.setWindowTitle(getTitleFromHtml(html));
      if (pendingSave.current) clearTimeout(pendingSave.current);
      pendingSave.current = setTimeout(() => {
        pendingSave.current = null;
        onChange(note.id, html);
      }, 400);
    },
    editorProps: {
      attributes: {
        class: 'h-full text-gray-900 text-[15px] leading-relaxed',
      },
      handlePaste: (view, event) => {
        const files = Array.from(event.clipboardData?.files ?? []);
        if (!files.some(f => f.type.startsWith('image/'))) return false;
        event.preventDefault();
        void insertImageFilesIntoView(view, files);
        return true;
      },
      handleDrop: (view, event) => {
        const dt = (event as DragEvent).dataTransfer;
        const files = Array.from(dt?.files ?? []);
        if (!files.some(f => f.type.startsWith('image/'))) return false;
        event.preventDefault();
        const coords = view.posAtCoords({ left: (event as DragEvent).clientX, top: (event as DragEvent).clientY });
        void insertImageFilesIntoView(view, files, coords?.pos);
        return true;
      },
    },
  });

  // Sync editor content when switching to a different note
  useEffect(() => {
    if (!editor) return;
    if (pendingSave.current) {
      clearTimeout(pendingSave.current);
      pendingSave.current = null;
    }
    skipNextUpdate.current = true;
    editor.commands.setContent(note.content || '<p></p>');
    setProgress(countTasks(note.content || ''));
    editor.commands.focus('end');
    if (isWindow) api.setWindowTitle(getTitleFromHtml(note.content || ''));
  }, [note.id, editor]);

  // Apply external content changes (e.g. edits from a note window) without
  // disturbing the user if they are currently typing in this editor.
  useEffect(() => {
    if (!editor) return;
    if (pendingSave.current) return;
    const incoming = note.content || '<p></p>';
    if (editor.getHTML() === incoming) return;
    skipNextUpdate.current = true;
    editor.commands.setContent(incoming);
  }, [note.content, editor]);

  if (!editor) return null;

  const openInWindow = () => api.openNoteWindow(note.id);

  const togglePin = async () => {
    const next = await api.toggleAlwaysOnTop(pinned);
    setPinned(next);
  };

  return (
    <div className="flex flex-col h-full" style={{ background: '#fef9c3' }}>

      {/* Custom title bar — only in frameless window mode */}
      {isWindow && (
        <div
          data-tauri-drag-region
          className="shrink-0 flex items-center justify-end gap-1 px-2"
          style={{ height: 32, background: '#fef08a', borderBottom: '1px solid #fde047' }}
        >
          {/* Pin / always-on-top */}
          <button
            className="w-7 h-7 flex items-center justify-center rounded transition-colors"
            style={{ color: pinned ? '#92400e' : '#78716c', background: pinned ? '#fcd34d' : 'transparent' }}
            onMouseDown={e => e.preventDefault()}
            onClick={togglePin}
            title={pinned ? 'Unpin window' : 'Keep on top'}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill={pinned ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="12" y1="17" x2="12" y2="22" />
              <path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z" />
            </svg>
          </button>

          {/* Close */}
          <button
            className="w-5 h-5 flex items-center justify-center rounded-full text-white transition-colors"
            style={{ background: '#f87171' }}
            onMouseDown={e => e.preventDefault()}
            onClick={() => api.closeWindow()}
            title="Close"
            onMouseEnter={e => (e.currentTarget.style.background = '#ef4444')}
            onMouseLeave={e => (e.currentTarget.style.background = '#f87171')}
          >
            <svg width="8" height="8" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
              <line x1="2" y1="2" x2="8" y2="8" /><line x1="8" y1="2" x2="2" y2="8" />
            </svg>
          </button>
        </div>
      )}

      {progress.total > 0 && (
        <div className="shrink-0 px-7 pt-4">
          <div className="flex items-center justify-between text-xs mb-1" style={{ color: '#92400e' }}>
            <span>{progress.done}/{progress.total} erledigt</span>
          </div>
          <div className="h-1.5 rounded-full overflow-hidden" style={{ background: '#fde68a' }}>
            <div className="h-full rounded-full" style={{ width: `${(progress.done / progress.total) * 100}%`, background: '#ca8a04' }} />
          </div>
        </div>
      )}

      {/* Scrollable content area */}
      <div className="flex-1 overflow-auto px-7 py-6">
        <EditorContent editor={editor} className="h-full" />
      </div>

      {/* Bottom toolbar */}
      <div
        className="shrink-0 flex items-center gap-0.5 px-3 py-2 border-t"
        style={{ background: '#fef08a', borderColor: '#fde047' }}
      >
        <ToolbarBtn onClick={() => editor.chain().focus().toggleBold().run()} active={editor.isActive('bold')} title="Bold">
          <span className="font-bold">B</span>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleItalic().run()} active={editor.isActive('italic')} title="Italic">
          <span className="italic">I</span>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleUnderline().run()} active={editor.isActive('underline')} title="Underline">
          <span className="underline">U</span>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleStrike().run()} active={editor.isActive('strike')} title="Strikethrough">
          <span className="line-through">S</span>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleTaskList().run()} active={editor.isActive('taskList')} title="Aufgabenliste">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="3 7 5 9 9 5" /><polyline points="3 17 5 19 9 15" /><line x1="13" y1="7" x2="21" y2="7" /><line x1="13" y1="17" x2="21" y2="17" />
          </svg>
        </ToolbarBtn>

        <div className="w-px h-5 bg-yellow-400 mx-1" />

        <ToolbarBtn onClick={() => editor.chain().focus().toggleBulletList().run()} active={editor.isActive('bulletList')} title="Bullet list">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="9" y1="6" x2="20" y2="6" /><line x1="9" y1="12" x2="20" y2="12" /><line x1="9" y1="18" x2="20" y2="18" />
            <circle cx="4" cy="6" r="1.5" fill="currentColor" stroke="none" />
            <circle cx="4" cy="12" r="1.5" fill="currentColor" stroke="none" />
            <circle cx="4" cy="18" r="1.5" fill="currentColor" stroke="none" />
          </svg>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleOrderedList().run()} active={editor.isActive('orderedList')} title="Numbered list">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="10" y1="6" x2="21" y2="6" /><line x1="10" y1="12" x2="21" y2="12" /><line x1="10" y1="18" x2="21" y2="18" />
            <path d="M4 6h1v4" stroke="currentColor" strokeWidth="1.8" fill="none" />
            <path d="M4 10h2" stroke="currentColor" strokeWidth="1.8" />
            <path d="M6 18H4c0-1 2-2 2-3s-1-1.5-2-1" stroke="currentColor" strokeWidth="1.8" fill="none" />
          </svg>
        </ToolbarBtn>

        <div className="w-px h-5 bg-yellow-400 mx-1" />

        <ToolbarBtn onClick={() => fileInputRef.current?.click()} title="Insert image">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <circle cx="8.5" cy="9" r="1.5" />
            <path d="m21 16-5-5L5 21" />
          </svg>
        </ToolbarBtn>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          multiple
          className="hidden"
          onChange={async e => {
            const files = Array.from(e.target.files ?? []);
            if (files.length) await insertImageFilesIntoEditor(editor, files);
            e.target.value = '';
          }}
        />

        {!isWindow && (
          <>
            <div className="w-px h-5 bg-yellow-400 mx-1" />
            <ToolbarBtn onClick={openInWindow} title="Open in new window">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="10" height="10" rx="1.5" />
                <rect x="11" y="11" width="10" height="10" rx="1.5" />
              </svg>
            </ToolbarBtn>
          </>
        )}
      </div>
    </div>
  );
}
