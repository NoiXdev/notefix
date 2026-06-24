import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import CodeEditor from 'react-simple-code-editor';
import { useEditor, EditorContent, type Editor } from '@tiptap/react';
import type { EditorView } from '@tiptap/pm/view';
import { DOMSerializer } from '@tiptap/pm/model';
import { highlightMarkdown } from '../mdHighlight';
import { selectionToCopy, type CopyFormat } from '../copyFormat';
import StarterKit from '@tiptap/starter-kit';
import Underline from '@tiptap/extension-underline';
import Placeholder from '@tiptap/extension-placeholder';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import { countTasks } from '../tasks';
import { ResizableImage } from './ResizableImage';
import { LinkPreview, LinkPreviewCtx, type LinkDisplay } from './LinkPreviewNode';
import { isBareUrl } from '../linkMeta';
import type { Note } from '../types';
import { api } from '../api';
import { saveImageFile } from '../saveImage';
import { toDateInputValue, fromDateInputValue } from '../dates';
import { htmlToMarkdown, markdownToHtml } from '../markdown';
import HistoryModal from './HistoryModal';
import { mdCursor, richCounts } from '../editorStatus';

async function insertImageFilesIntoView(view: EditorView, files: File[], noteId: string, pos?: number): Promise<void> {
  const images = files.filter(f => f.type.startsWith('image/'));
  if (!images.length) return;
  for (const file of images) {
    const src = await saveImageFile(noteId, file);
    const node = view.state.schema.nodes.image?.create({ src });
    if (!node) continue;
    const insertAt = typeof pos === 'number' ? pos : view.state.selection.to;
    view.dispatch(view.state.tr.insert(insertAt, node));
  }
}

async function insertImageFilesIntoEditor(editor: Editor, files: File[], noteId: string): Promise<void> {
  const images = files.filter(f => f.type.startsWith('image/'));
  if (!images.length) return;
  for (const file of images) {
    const src = await saveImageFile(noteId, file);
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
  onSetDue?: (id: string, dueAt: number | null) => void;
  autosaveDelay?: number;
  linkPreviewEnabled?: boolean;
  linkPreviewMode?: LinkDisplay;
  copyFormat?: CopyFormat;
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

export default function NoteEditor({ note, onChange, isWindow = false, onSetDue, autosaveDelay = 400, linkPreviewEnabled = true, linkPreviewMode = 'card', copyFormat = 'md' }: Props) {
  const { t } = useTranslation();
  const [pinned, setPinned] = useState(false);
  const [progress, setProgress] = useState({ done: 0, total: 0 });
  const pendingSave = useRef<ReturnType<typeof setTimeout> | null>(null);
  const skipNextUpdate = useRef(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [mdMode, setMdMode] = useState(false);
  const [mdText, setMdText] = useState('');
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved'>('saved');
  const [lastSavedAt, setLastSavedAt] = useState<number | null>(null);
  const [selStart, setSelStart] = useState(0);
  const [rich, setRich] = useState({ words: 0, chars: 0, sel: 0 });
  const delayRef = useRef(autosaveDelay);
  delayRef.current = autosaveDelay;
  const lpRef = useRef({ enabled: linkPreviewEnabled, mode: linkPreviewMode });
  lpRef.current = { enabled: linkPreviewEnabled ?? true, mode: linkPreviewMode ?? 'card' };
  const copyRef = useRef<CopyFormat>(copyFormat ?? 'md');
  copyRef.current = copyFormat ?? 'md';
  // handleDOMEvents is created once in the useEditor config, so it reads the
  // copy/cut handler via this ref to always pick up the current copyFormat.
  const doCopyRef = useRef<(view: EditorView, event: ClipboardEvent, cut: boolean) => boolean>(() => false);

  const editor = useEditor({
    extensions: [
      StarterKit,
      Underline,
      Placeholder.configure({ placeholder: t('editor.placeholder') }),
      TaskList,
      TaskItem.configure({ nested: true }),
      ResizableImage.configure({ inline: false, allowBase64: true }),
      LinkPreview,
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
      setSaveState('saving');
      pendingSave.current = setTimeout(() => {
        pendingSave.current = null;
        onChange(note.id, html);
        setSaveState('saved');
        setLastSavedAt(Date.now());
      }, delayRef.current);
    },
    editorProps: {
      attributes: {
        class: 'h-full text-gray-900 text-[15px] leading-relaxed',
      },
      handlePaste: (view, event) => {
        const text = event.clipboardData?.getData('text/plain') ?? '';
        if (lpRef.current.enabled && isBareUrl(text)) {
          event.preventDefault();
          view.dispatch(view.state.tr.replaceSelectionWith(
            view.state.schema.nodes.linkPreview.create({ href: text.trim(), display: lpRef.current.mode })
          ).scrollIntoView());
          return true;
        }
        const files = Array.from(event.clipboardData?.files ?? []);
        if (!files.some(f => f.type.startsWith('image/'))) return false;
        event.preventDefault();
        void insertImageFilesIntoView(view, files, note.id);
        return true;
      },
      handleDrop: (view, event) => {
        const dt = (event as DragEvent).dataTransfer;
        const files = Array.from(dt?.files ?? []);
        if (!files.some(f => f.type.startsWith('image/'))) return false;
        event.preventDefault();
        const coords = view.posAtCoords({ left: (event as DragEvent).clientX, top: (event as DragEvent).clientY });
        void insertImageFilesIntoView(view, files, note.id, coords?.pos);
        return true;
      },
      handleDOMEvents: {
        copy: (view, event) => doCopyRef.current(view, event as ClipboardEvent, false),
        cut: (view, event) => doCopyRef.current(view, event as ClipboardEvent, true),
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
    setMdMode(false);
    setSaveState('saved');
    setLastSavedAt(null);
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

  // Track Tiptap selection/content to drive the rich-mode status bar.
  useEffect(() => {
    if (!editor) return;
    const update = () => {
      const sel = editor.state?.selection;
      if (!sel || !editor.state?.doc) return;
      setRich(richCounts(editor.state.doc.textContent ?? '', sel.to - sel.from));
    };
    update();
    if (typeof editor.on !== 'function') return;
    editor.on('update', update);
    editor.on('selectionUpdate', update);
    return () => { editor.off?.('update', update); editor.off?.('selectionUpdate', update); };
  }, [editor]);

  if (!editor) return null;

  // Cursor tracking for the md-mode status bar.
  const onCur = (e: { target: EventTarget | null }) => setSelStart((e.target as HTMLTextAreaElement).selectionStart ?? 0);

  function doCopy(view: EditorView, event: ClipboardEvent, cut: boolean): boolean {
    if (copyRef.current === 'richtext') return false;
    const sel = view.state.selection;
    if (sel.empty) return false;
    const div = document.createElement('div');
    div.appendChild(DOMSerializer.fromSchema(view.state.schema).serializeFragment(view.state.doc.slice(sel.from, sel.to).content));
    event.clipboardData?.setData('text/plain', selectionToCopy(div.innerHTML, copyRef.current));
    event.preventDefault();
    if (cut) view.dispatch(view.state.tr.deleteSelection());
    return true;
  }
  doCopyRef.current = doCopy;

  const flushSave = () => {
    if (!pendingSave.current) return;
    clearTimeout(pendingSave.current);
    pendingSave.current = null;
    onChange(note.id, mdMode ? markdownToHtml(mdText) : editor.getHTML());
    setSaveState('saved');
    setLastSavedAt(Date.now());
  };

  const restore = (content: string) => {
    skipNextUpdate.current = true;
    editor.commands.setContent(content);
    onChange(note.id, content);
    setMdMode(false);
    setHistoryOpen(false);
  };

  const toggleMd = () => {
    if (mdMode) {
      skipNextUpdate.current = true;
      editor.commands.setContent(markdownToHtml(mdText));
      setMdMode(false);
    } else {
      setMdText(htmlToMarkdown(editor.getHTML()));
      setMdMode(true);
    }
  };

  const onMdChange = (value: string) => {
    setMdText(value);
    if (pendingSave.current) clearTimeout(pendingSave.current);
    setSaveState('saving');
    pendingSave.current = setTimeout(() => {
      pendingSave.current = null;
      onChange(note.id, markdownToHtml(value));
      setSaveState('saved');
      setLastSavedAt(Date.now());
    }, delayRef.current);
  };

  const openInWindow = () => api.openNoteWindow(note.id);

  const togglePin = async () => {
    const next = await api.toggleAlwaysOnTop(pinned);
    setPinned(next);
  };

  const statusText = mdMode
    ? t('editor.status.md', mdCursor(mdText, selStart))
    : rich.sel > 0
      ? t('editor.status.richSel', rich)
      : t('editor.status.rich', rich);

  return (
    <div className="flex flex-col h-full relative" style={{ background: '#fef9c3' }}>
      {/* Top-right cluster: live status (words/chars or Ln/Col) + autosave indicator */}
      <div className={`absolute right-2 ${isWindow ? 'top-10' : 'top-1'} z-10 flex items-center gap-2`}>
        <span className="font-mono text-[11px] text-gray-500 select-none pointer-events-none whitespace-nowrap">{statusText}</span>
        <button
          onClick={flushSave}
          title={saveState === 'saving' ? t('editor.saving') : lastSavedAt ? t('editor.savedAt', { time: new Date(lastSavedAt).toLocaleTimeString() }) : t('editor.saved')}
          className="w-6 h-6 flex items-center justify-center rounded text-amber-700/70 hover:text-amber-800"
          aria-label={t('editor.save')}
        >
          {saveState === 'saving' ? (
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56" /></svg>
          ) : (
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" /><polyline points="17 21 17 13 7 13 7 21" /><polyline points="7 3 7 8 15 8" /></svg>
          )}
        </button>
      </div>

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
            title={pinned ? t('editor.unpin') : t('editor.pin')}
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
            title={t('editor.close')}
            onMouseEnter={e => (e.currentTarget.style.background = '#ef4444')}
            onMouseLeave={e => (e.currentTarget.style.background = '#f87171')}
          >
            <svg width="8" height="8" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
              <line x1="2" y1="2" x2="8" y2="8" /><line x1="8" y1="2" x2="2" y2="8" />
            </svg>
          </button>
        </div>
      )}

      {/* Resize grip — frameless windows have no native edges on macOS */}
      {isWindow && (
        <div
          onMouseDown={() => { void api.startResize(); }}
          title={t('editor.resize')}
          style={{ position: 'fixed', right: 0, bottom: 0, width: 18, height: 18, cursor: 'nwse-resize', zIndex: 50 }}
        />
      )}

      {onSetDue && (
        <div className="shrink-0 px-7 pt-3 flex items-center gap-2 text-xs" style={{ color: '#92400e' }}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="4" width="18" height="18" rx="2" /><line x1="3" y1="10" x2="21" y2="10" /><line x1="8" y1="2" x2="8" y2="6" /><line x1="16" y1="2" x2="16" y2="6" /></svg>
          <input
            type="date"
            aria-label={t('editor.dueDate')}
            value={toDateInputValue(note.dueAt)}
            onChange={e => onSetDue(note.id, fromDateInputValue(e.target.value))}
            className="bg-transparent outline-none"
            style={{ color: '#92400e' }}
          />
          {note.dueAt != null && (
            <button onMouseDown={e => e.preventDefault()} onClick={() => onSetDue(note.id, null)} title={t('editor.clearDue')} className="px-1">×</button>
          )}
        </div>
      )}

      {progress.total > 0 && (
        <div className="shrink-0 px-7 pt-4">
          <div className="flex items-center justify-between text-xs mb-1" style={{ color: '#92400e' }}>
            <span>{t('editor.progress', { done: progress.done, total: progress.total })}</span>
          </div>
          <div className="h-1.5 rounded-full overflow-hidden" style={{ background: '#fde68a' }}>
            <div className="h-full rounded-full" style={{ width: `${(progress.done / progress.total) * 100}%`, background: '#ca8a04' }} />
          </div>
        </div>
      )}

      {/* Scrollable content area */}
      <div className="flex-1 overflow-auto px-7 py-6">
        {mdMode
          ? <CodeEditor
              value={mdText}
              onValueChange={onMdChange}
              highlight={highlightMarkdown}
              onKeyUp={onCur}
              onClick={onCur}
              padding={0}
              textareaClassName="outline-none"
              className="w-full h-full font-mono text-sm text-gray-900"
              style={{ fontFamily: 'monospace', fontSize: 14, minHeight: '100%' }}
            />
          : <LinkPreviewCtx.Provider value={{ enabled: linkPreviewEnabled ?? true, mode: linkPreviewMode ?? 'card' }}>
              <EditorContent editor={editor} className="h-full" />
            </LinkPreviewCtx.Provider>}
      </div>

      {/* Bottom toolbar */}
      <div
        className="shrink-0 flex items-center gap-0.5 px-3 py-2 border-t"
        style={{ background: '#fef08a', borderColor: '#fde047' }}
      >
        <ToolbarBtn onClick={() => editor.chain().focus().toggleBold().run()} active={editor.isActive('bold')} title={t('editor.bold')}>
          <span className="font-bold">B</span>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleItalic().run()} active={editor.isActive('italic')} title={t('editor.italic')}>
          <span className="italic">I</span>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleUnderline().run()} active={editor.isActive('underline')} title={t('editor.underline')}>
          <span className="underline">U</span>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleStrike().run()} active={editor.isActive('strike')} title={t('editor.strikethrough')}>
          <span className="line-through">S</span>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleTaskList().run()} active={editor.isActive('taskList')} title={t('editor.taskList')}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="3 7 5 9 9 5" /><polyline points="3 17 5 19 9 15" /><line x1="13" y1="7" x2="21" y2="7" /><line x1="13" y1="17" x2="21" y2="17" />
          </svg>
        </ToolbarBtn>

        <div className="w-px h-5 bg-yellow-400 mx-1" />

        <ToolbarBtn onClick={() => editor.chain().focus().toggleBulletList().run()} active={editor.isActive('bulletList')} title={t('editor.bulletList')}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="9" y1="6" x2="20" y2="6" /><line x1="9" y1="12" x2="20" y2="12" /><line x1="9" y1="18" x2="20" y2="18" />
            <circle cx="4" cy="6" r="1.5" fill="currentColor" stroke="none" />
            <circle cx="4" cy="12" r="1.5" fill="currentColor" stroke="none" />
            <circle cx="4" cy="18" r="1.5" fill="currentColor" stroke="none" />
          </svg>
        </ToolbarBtn>
        <ToolbarBtn onClick={() => editor.chain().focus().toggleOrderedList().run()} active={editor.isActive('orderedList')} title={t('editor.numberedList')}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="10" y1="6" x2="21" y2="6" /><line x1="10" y1="12" x2="21" y2="12" /><line x1="10" y1="18" x2="21" y2="18" />
            <path d="M4 6h1v4" stroke="currentColor" strokeWidth="1.8" fill="none" />
            <path d="M4 10h2" stroke="currentColor" strokeWidth="1.8" />
            <path d="M6 18H4c0-1 2-2 2-3s-1-1.5-2-1" stroke="currentColor" strokeWidth="1.8" fill="none" />
          </svg>
        </ToolbarBtn>

        <div className="w-px h-5 bg-yellow-400 mx-1" />

        <ToolbarBtn onClick={() => fileInputRef.current?.click()} title={t('editor.insertImage')}>
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
            if (files.length) await insertImageFilesIntoEditor(editor, files, note.id);
            e.target.value = '';
          }}
        />

        <div className="w-px h-5 bg-yellow-400 mx-1" />
        <ToolbarBtn onClick={() => setHistoryOpen(true)} title={t('editor.history')}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 3v5h5" /><path d="M3.05 13A9 9 0 1 0 6 5.3L3 8" /><path d="M12 7v5l3 2" />
          </svg>
        </ToolbarBtn>
        <ToolbarBtn onClick={toggleMd} active={mdMode} title={t('editor.markdown')}>
          <span className="font-mono text-xs">&lt;/&gt;</span>
        </ToolbarBtn>

        {!isWindow && (
          <>
            <div className="w-px h-5 bg-yellow-400 mx-1" />
            <ToolbarBtn onClick={openInWindow} title={t('editor.openInWindow')}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="10" height="10" rx="1.5" />
                <rect x="11" y="11" width="10" height="10" rx="1.5" />
              </svg>
            </ToolbarBtn>
          </>
        )}
      </div>
      {historyOpen && <HistoryModal noteId={note.id} onRestore={restore} onClose={() => setHistoryOpen(false)} />}
    </div>
  );
}
