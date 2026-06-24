import { useTranslation } from 'react-i18next';
interface Props {
  onBase64: () => void;
  onBundle: () => void;
  onCancel: () => void;
}
export default function ExportDialog({ onBase64, onBundle, onCancel }: Props) {
  const { t } = useTranslation();
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('dialogs.export.title')}</h2>
        <p className="text-gray-400 text-sm mb-4">{t('dialogs.export.message')}</p>
        <div className="flex flex-col gap-2">
          <button onClick={onBase64} className="px-3 py-2 rounded text-sm font-medium text-left" style={{ background: '#fde047', color: '#1c1917' }}>{t('dialogs.export.embed')}</button>
          <button onClick={onBundle} className="px-3 py-2 rounded text-sm font-medium text-left bg-gray-800 text-gray-100 hover:bg-gray-700">{t('dialogs.export.bundle')}</button>
          <button onClick={onCancel} className="px-3 py-1.5 rounded text-sm text-gray-400 hover:bg-gray-800 self-end">{t('dialogs.export.cancel')}</button>
        </div>
      </div>
    </div>
  );
}
