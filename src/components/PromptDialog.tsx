import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  title: string;
  confirmLabel: string;
  initialValue?: string;
  placeholder?: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

/**
 * In-app text-input dialog. Replaces window.prompt(), which does nothing in
 * Tauri's WKWebView (it returns null without showing a panel).
 */
export default function PromptDialog({ title, confirmLabel, initialValue = '', placeholder, onSubmit, onCancel }: Props) {
  const { t } = useTranslation();
  const [value, setValue] = useState(initialValue);
  const submit = () => { const v = value.trim(); if (v) onSubmit(v); };
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-3">{title}</h2>
        <input
          autoFocus
          value={value}
          placeholder={placeholder}
          onChange={e => setValue(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') submit(); else if (e.key === 'Escape') onCancel(); }}
          className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-yellow-400 mb-4"
        />
        <div className="flex justify-end gap-2">
          <button onClick={onCancel} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">{t('dialogs.confirm.cancel')}</button>
          <button onClick={submit} disabled={!value.trim()} className="px-3 py-1.5 rounded text-sm font-medium disabled:opacity-40" style={{ background: '#fde047', color: '#1c1917' }}>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}
