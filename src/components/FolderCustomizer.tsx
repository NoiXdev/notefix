import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import EmojiPicker, { Theme } from 'emoji-picker-react';
import IconCombobox from './IconCombobox';
import { NOTE_COLORS } from '../colors';
import type { Folder } from '../types';

type Mode = 'standard' | 'fa' | 'emoji';

function modeOf(icon: string): Mode {
  if (!icon) return 'standard';
  return icon.startsWith('fa:') ? 'fa' : 'emoji';
}

interface Props {
  x: number;
  y: number;
  folder: Folder;
  onSetIcon: (icon: string) => void;
  onSetColor: (color: string) => void;
  onClose: () => void;
}

const MODES: { value: Mode; labelKey: string }[] = [
  { value: 'standard', labelKey: 'folder.modeStandard' },
  { value: 'fa', labelKey: 'folder.modeFa' },
  { value: 'emoji', labelKey: 'folder.modeEmoji' },
];

export default function FolderCustomizer({ x, y, folder, onSetIcon, onSetColor, onClose }: Props) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<Mode>(modeOf(folder.icon));

  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const close = () => onCloseRef.current();
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
    const id = setTimeout(() => {
      window.addEventListener('click', close);
      window.addEventListener('contextmenu', close);
    }, 0);
    window.addEventListener('keydown', onKey);
    return () => {
      clearTimeout(id);
      window.removeEventListener('click', close);
      window.removeEventListener('contextmenu', close);
      window.removeEventListener('keydown', onKey);
    };
  }, []);

  const selectMode = (m: Mode) => {
    setMode(m);
    if (m === 'standard') onSetIcon('');
  };

  return (
    <div
      className="fixed z-50 w-72 p-3 rounded-md bg-gray-900 border border-gray-700 shadow-lg text-gray-200"
      style={{ left: Math.min(x, window.innerWidth - 300), top: Math.min(y, window.innerHeight - 460) }}
      onClick={e => e.stopPropagation()}
      onContextMenu={e => { e.preventDefault(); e.stopPropagation(); }}
    >
      <div className="text-xs font-semibold text-gray-400 mb-1">{t('folder.icon')}</div>
      <div className="flex gap-1 mb-2">
        {MODES.map(m => (
          <button
            key={m.value}
            onClick={() => selectMode(m.value)}
            className={`flex-1 px-2 py-1 rounded text-xs ${mode === m.value ? 'bg-yellow-400 text-gray-900' : 'bg-gray-800 text-gray-300 hover:bg-gray-700'}`}
          >
            {t(m.labelKey)}
          </button>
        ))}
      </div>

      {mode === 'standard' && <p className="text-xs text-gray-500 mb-3">{t('folder.standardHint')}</p>}
      {mode === 'fa' && <div className="mb-3"><IconCombobox value={folder.icon} onPick={onSetIcon} /></div>}
      {mode === 'emoji' && (
        <div className="mb-3">
          <EmojiPicker theme={Theme.DARK} width="100%" height={300} onEmojiClick={d => onSetIcon(d.emoji)} />
        </div>
      )}

      <div className="text-xs font-semibold text-gray-400 mb-1">{t('folder.color')}</div>
      <div className="flex items-center gap-1.5">
        <button onClick={() => onSetColor('')} aria-label={t('folder.noColor')} title={t('folder.noColorShort')} className="w-5 h-5 rounded-full border border-gray-600 text-gray-400 flex items-center justify-center text-[10px]">×</button>
        {NOTE_COLORS.map(c => (
          <button key={c} onClick={() => onSetColor(c)} aria-label={t('folder.colorLabel', { color: c })} className="w-5 h-5 rounded-full" style={{ background: c, outline: folder.color === c ? '2px solid #fff' : 'none', outlineOffset: '1px' }} />
        ))}
      </div>
    </div>
  );
}
