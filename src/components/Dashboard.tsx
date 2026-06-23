import type { ReactNode } from 'react';
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core';
import { SortableContext, useSortable, verticalListSortingStrategy, arrayMove } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { Note, Stats, Folder } from '../types';
import type { DashboardWidget } from '../hooks/useSettings';
import { WIDGETS, WIDGET_KEYS } from '../dashboardWidgets';

interface Props {
  notes: Note[];
  folders: Folder[];
  stats: Stats | null;
  layout: DashboardWidget[];
  editMode: boolean;
  onSelectNote: (id: string) => void;
  onCreateNote: () => void;
  onChangeLayout: (layout: DashboardWidget[]) => void;
  onToggleEdit: () => void;
}

function Card({ id, title, editMode, width, onRemove, onToggleWidth, handleProps, children }: { id: string; title: string; editMode: boolean; width: 1 | 2; onRemove: (id: string) => void; onToggleWidth: (id: string) => void; handleProps?: Record<string, unknown>; children: ReactNode }) {
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-3 shadow-sm">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-gray-700">{title}</h3>
        {editMode && (
          <div className="flex items-center gap-1">
            <button {...handleProps} title="Verschieben" className="px-1 text-gray-400 cursor-grab">⠿</button>
            <button onClick={() => onToggleWidth(id)} title="Breite" className="px-1 text-gray-400 hover:text-gray-700 text-xs">{width === 2 ? '2×' : '1×'}</button>
            <button onClick={() => onRemove(id)} title="Entfernen" className="px-1 text-gray-400 hover:text-red-500">×</button>
          </div>
        )}
      </div>
      {children}
    </div>
  );
}

function SortableCard({ id, title, editMode, width, onRemove, onToggleWidth, children }: { id: string; title: string; editMode: boolean; width: 1 | 2; onRemove: (id: string) => void; onToggleWidth: (id: string) => void; children: ReactNode }) {
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id });
  return (
    <div ref={setNodeRef} style={{ transform: CSS.Transform.toString(transform), transition }} className={width === 2 ? 'col-span-2' : ''}>
      <Card id={id} title={title} editMode={editMode} width={width} onRemove={onRemove} onToggleWidth={onToggleWidth} handleProps={{ ...attributes, ...listeners }}>
        {children}
      </Card>
    </div>
  );
}

export default function Dashboard({ notes, folders, stats, layout, editMode, onSelectNote, onCreateNote, onChangeLayout, onToggleEdit }: Props) {
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));
  const ctx = { notes, folders, stats, onSelectNote, onCreateNote };
  const active = layout.filter(w => WIDGETS[w.key]);
  const available = WIDGET_KEYS.filter(k => !active.some(w => w.key === k));
  const remove = (key: string) => onChangeLayout(active.filter(w => w.key !== key));
  const setWidth = (key: string) => onChangeLayout(active.map(w => w.key === key ? { ...w, w: (w.w === 1 ? 2 : 1) as 1 | 2 } : w));
  const onDragEnd = (e: DragEndEvent) => {
    const { active: a, over } = e;
    if (over && a.id !== over.id) {
      onChangeLayout(arrayMove(active, active.findIndex(w => w.key === a.id), active.findIndex(w => w.key === over.id)));
    }
  };

  return (
    <div className="h-full overflow-y-auto p-6" style={{ background: '#fef9c3' }}>
      <div className="flex items-center justify-between mb-4 flex-wrap gap-2">
        <h1 className="text-2xl font-bold text-gray-900">Dashboard</h1>
        <div className="flex items-center gap-2 flex-wrap">
          {editMode && available.map(k => (
            <button key={k} onClick={() => onChangeLayout([...active, { key: k, w: 1 }])} className="px-2 py-1 rounded text-xs bg-white border border-gray-300 hover:bg-gray-100">+ {WIDGETS[k].label}</button>
          ))}
          <button onClick={onToggleEdit} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: editMode ? '#fde047' : 'white', border: '1px solid #e7d27a', color: '#1c1917' }}>{editMode ? 'Fertig' : 'Bearbeiten'}</button>
        </div>
      </div>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={active.map(w => w.key)} strategy={verticalListSortingStrategy}>
          <div className="grid grid-cols-2 gap-4">
            {active.map(w => (
              <SortableCard key={w.key} id={w.key} title={WIDGETS[w.key].label} editMode={editMode} width={w.w} onRemove={remove} onToggleWidth={setWidth}>
                {WIDGETS[w.key].render(ctx)}
              </SortableCard>
            ))}
          </div>
        </SortableContext>
      </DndContext>
      {active.length === 0 && <p className="text-sm text-gray-500">Keine Widgets. Klicke „Bearbeiten" und füge welche hinzu.</p>}
    </div>
  );
}
