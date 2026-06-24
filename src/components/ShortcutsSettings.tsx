import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SHORTCUT_ACTIONS, resolveBindings, eventToCombo, comboLabel } from '../shortcuts';

export default function ShortcutsSettings({ value, onChange }: { value: Record<string, string>; onChange: (v: Record<string, string>) => void }) {
  const { t } = useTranslation();
  const [recording, setRecording] = useState<string | null>(null);
  const bindings = resolveBindings(value);

  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const combo = eventToCombo(e);
      if (!combo) return; // auf Nicht-Modifier-Taste warten
      onChange({ ...value, [recording]: combo });
      setRecording(null);
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [recording, value, onChange]);

  const counts: Record<string, number> = {};
  for (const a of SHORTCUT_ACTIONS) counts[bindings[a.id]] = (counts[bindings[a.id]] ?? 0) + 1;
  const reset = (id: string) => { const v = { ...value }; delete v[id]; onChange(v); };

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900 mb-1">{t('shortcuts.title')}</h1>
      <p className="text-sm text-gray-500 mb-6">{t('shortcuts.help')}</p>
      <div className="flex flex-col gap-2 max-w-lg">
        {SHORTCUT_ACTIONS.map(a => {
          const combo = bindings[a.id];
          const conflict = counts[combo] > 1;
          const overridden = value[a.id] != null;
          return (
            <div key={a.id} className="flex items-center justify-between gap-3 text-sm text-gray-800 border-b border-yellow-200 pb-1.5">
              <span>{t(a.labelKey)}{conflict && <span className="ml-2 text-xs text-red-600">{t('shortcuts.conflict')}</span>}</span>
              <div className="flex items-center gap-2">
                <kbd className="px-2 py-0.5 rounded bg-white border text-xs" style={{ borderColor: conflict ? '#dc2626' : '#e7d27a' }}>
                  {recording === a.id ? t('shortcuts.pressKey') : comboLabel(combo)}
                </kbd>
                <button onClick={() => setRecording(a.id)} className="px-2 py-0.5 rounded text-xs border" style={{ borderColor: '#e7d27a' }}>{t('shortcuts.change')}</button>
                {overridden && <button onClick={() => reset(a.id)} title={t('shortcuts.resetOne')} className="px-2 py-0.5 rounded text-xs text-gray-500 hover:text-gray-800">↺</button>}
              </div>
            </div>
          );
        })}
        <button onClick={() => onChange({})} className="self-start mt-2 px-4 py-1.5 rounded text-sm font-medium border" style={{ borderColor: '#e7d27a', color: '#1c1917' }}>{t('shortcuts.resetAll')}</button>
      </div>
    </div>
  );
}
