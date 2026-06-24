import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ExportFormat } from '../export/exporters';

export default function ExportFormatModal({ onExport, onCancel }: { onExport: (f: ExportFormat, mdBundle: boolean) => void; onCancel: () => void }) {
  const { t } = useTranslation();
  const [mdBundle, setMdBundle] = useState(false);
  const formats: { key: ExportFormat; label: string }[] = [
    { key: 'md', label: 'Markdown' },
    { key: 'txt', label: t('export.text') },
    { key: 'pdf', label: 'PDF' },
    { key: 'jpg', label: 'JPG' },
    { key: 'doc', label: 'Word' },
  ];
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-3">{t('export.title')}</h2>
        <div className="flex flex-col gap-2">
          {formats.map(f => (
            <button key={f.key} onClick={() => onExport(f.key, mdBundle)} className="px-3 py-2 rounded text-sm font-medium text-left bg-gray-800 text-gray-100 hover:bg-gray-700">{f.label}</button>
          ))}
        </div>
        <label className="flex items-center gap-2 text-sm text-gray-300 mt-3">
          <input type="checkbox" checked={mdBundle} onChange={() => setMdBundle(v => !v)} />
          {t('export.mdBundle')}
        </label>
        <div className="flex justify-end mt-4">
          <button onClick={onCancel} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">{t('dialogs.export.cancel')}</button>
        </div>
      </div>
    </div>
  );
}
