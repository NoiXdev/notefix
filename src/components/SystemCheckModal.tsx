import { useTranslation } from 'react-i18next';
import type { SystemCheck } from '../systemChecks';

interface Props {
  problems: SystemCheck[];
  onOpenSettings: () => void;
  onClose: () => void;
}
export default function SystemCheckModal({ problems, onOpenSettings, onClose }: Props) {
  const { t } = useTranslation();
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onClose}>
      <div className="w-[26rem] rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('dialogs.systemCheck.title')}</h2>
        <p className="text-gray-400 text-sm mb-3">{t('dialogs.systemCheck.message')}</p>
        <ul className="mb-4 flex flex-col gap-1">
          {problems.map(p => (
            <li key={p.key} className="text-sm text-gray-200">
              <span className="text-red-400">●</span> {p.label}
              <span className="text-gray-500"> — {p.detail}</span>
            </li>
          ))}
        </ul>
        <div className="flex justify-end gap-2">
          <button onClick={onClose} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">{t('dialogs.systemCheck.close')}</button>
          <button onClick={onOpenSettings} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: '#fde047', color: '#1c1917' }}>{t('dialogs.systemCheck.openSettings')}</button>
        </div>
      </div>
    </div>
  );
}
