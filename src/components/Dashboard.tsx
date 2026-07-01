import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import GridLayout, { WidthProvider } from 'react-grid-layout';
import type { Layout } from 'react-grid-layout';
import type { NoteMeta, Stats, Folder } from '../types';
import type { DashboardWidget } from '../hooks/useSettings';
import { WIDGETS, WIDGET_KEYS, WIDGET_SIZES } from '../dashboardWidgets';

const RGL = WidthProvider(GridLayout);
const COLS = 12;
const ROW_H = 80;

interface Props {
  notes: NoteMeta[];
  folders: Folder[];
  stats: Stats | null;
  layout: DashboardWidget[];
  editMode: boolean;
  onSelectNote: (id: string) => void;
  onCreateNote: () => void;
  onChangeLayout: (layout: DashboardWidget[]) => void;
  onToggleEdit: () => void;
}

function Card({ id, title, editMode, onRemove, children }: { id: string; title: string; editMode: boolean; onRemove: (id: string) => void; children: ReactNode }) {
  const { t } = useTranslation();
  return (
    <div className="h-full flex flex-col rounded-lg border border-gray-200 bg-white p-3 shadow-sm overflow-hidden">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-gray-700 truncate">{title}</h3>
        {editMode && (
          <div className="flex items-center gap-1 shrink-0">
            <span className="drag-handle px-1 text-gray-400 cursor-grab" title={t('dashboard.move')}>⠿</span>
            <button onClick={() => onRemove(id)} title={t('dashboard.remove')} className="px-1 text-gray-400 hover:text-red-500">×</button>
          </div>
        )}
      </div>
      <div className="flex-1 overflow-auto">{children}</div>
    </div>
  );
}

export default function Dashboard({ notes, folders, stats, layout, editMode, onSelectNote, onCreateNote, onChangeLayout, onToggleEdit }: Props) {
  const { t } = useTranslation();
  const ctx = { notes, folders, stats, onSelectNote, onCreateNote };
  const active = layout.filter(w => WIDGETS[w.key]);
  const available = WIDGET_KEYS.filter(k => !active.some(w => w.key === k));

  const rglLayout: Layout[] = active.map(w => {
    const s = WIDGET_SIZES[w.key];
    return { i: w.key, x: w.x, y: w.y, w: w.w, h: w.h, minW: s.minW, minH: s.minH };
  });

  const persist = (l: Layout[]) => {
    onChangeLayout(l.filter(it => WIDGETS[it.i]).map(it => ({ key: it.i, x: it.x, y: it.y, w: it.w, h: it.h })));
  };

  const remove = (key: string) => onChangeLayout(active.filter(w => w.key !== key));
  const add = (key: string) => {
    const s = WIDGET_SIZES[key];
    onChangeLayout([...active, { key, x: 0, y: Infinity, w: s.w, h: s.h }]);
  };

  return (
    <div className="h-full overflow-y-auto p-6" style={{ background: '#fef9c3' }}>
      <div className="flex items-center justify-between mb-4 flex-wrap gap-2">
        <h1 className="text-2xl font-bold text-gray-900">{t('dashboard.title')}</h1>
        <div className="flex items-center gap-2 flex-wrap">
          {editMode && available.map(k => (
            <button key={k} onClick={() => add(k)} className="px-2 py-1 rounded text-xs bg-white border border-gray-300 hover:bg-gray-100">+ {t(WIDGETS[k].labelKey)}</button>
          ))}
          <button onClick={onToggleEdit} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: editMode ? '#fde047' : 'white', border: '1px solid #e7d27a', color: '#1c1917' }}>{editMode ? t('dashboard.done') : t('dashboard.edit')}</button>
        </div>
      </div>
      {active.length === 0 ? (
        <p className="text-sm text-gray-500">{t('dashboard.emptyHint')}</p>
      ) : (
        <RGL
          className="layout"
          layout={rglLayout}
          cols={COLS}
          rowHeight={ROW_H}
          margin={[16, 16]}
          compactType={null}
          preventCollision
          isDraggable={editMode}
          isResizable={editMode}
          draggableHandle=".drag-handle"
          onDragStop={persist}
          onResizeStop={persist}
        >
          {active.map(w => (
            <div key={w.key}>
              <Card id={w.key} title={t(WIDGETS[w.key].labelKey)} editMode={editMode} onRemove={remove}>
                {WIDGETS[w.key].render(ctx)}
              </Card>
            </div>
          ))}
        </RGL>
      )}
    </div>
  );
}
