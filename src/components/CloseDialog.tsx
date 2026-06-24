import { useState } from 'react';
import { useTranslation } from 'react-i18next';
interface Props {
  onMinimize: (remember: boolean) => void;
  onQuit: (remember: boolean) => void;
  onCancel: () => void;
}
export default function CloseDialog({ onMinimize, onQuit, onCancel }: Props) {
  const { t } = useTranslation();
  const [remember, setRemember] = useState(false);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('dialogs.close.title')}</h2>
        <p className="text-gray-400 text-sm mb-4">{t('dialogs.close.message')}</p>
        <label className="flex items-center gap-2 text-sm text-gray-300 mb-4">
          <input type="checkbox" checked={remember} onChange={() => setRemember(v => !v)} />
          {t('dialogs.close.remember')}
        </label>
        <div className="flex justify-end gap-2">
          <button onClick={onCancel} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">{t('dialogs.close.cancel')}</button>
          <button onClick={() => onMinimize(remember)} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: '#fde047', color: '#1c1917' }}>{t('dialogs.close.minimize')}</button>
          <button onClick={() => onQuit(remember)} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: '#dc2626', color: 'white' }}>{t('dialogs.close.quit')}</button>
        </div>
      </div>
    </div>
  );
}
