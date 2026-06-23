import type { ReactNode } from 'react';
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core';
import { SortableContext, useSortable, verticalListSortingStrategy, arrayMove } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { Note, Stats } from '../types';
import { WIDGETS, WIDGET_KEYS } from '../dashboardWidgets';

interface Props {
  notes: Note[];
  stats: Stats | null;
  layout: string[];
  editMode: boolean;
  onSelectNote: (id: string) => void;
  onChangeLayout: (layout: string[]) => void;
  onToggleEdit: () => void;
}

function Card({ id, title, editMode, onRemove, handleProps, children }: { id: string; title: string; editMode: boolean; onRemove: (id: string) => void; handleProps?: Record<string, unknown>; children: ReactNode }) {
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-3 shadow-sm">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-gray-700">{title}</h3>
        {editMode && (
          <div className="flex items-center gap-1">
            <button {...handleProps} title="Verschieben" className="px-1 text-gray-400 cursor-grab">⠿</button>
            <button onClick={() => onRemove(id)} title="Entfernen" className="px-1 text-gray-400 hover:text-red-500">×</button>
          </div>
        )}
      </div>
      {children}
    </div>
  );
}

function SortableCard({ id, title, editMode, onRemove, children }: { id: string; title: string; editMode: boolean; onRemove: (id: string) => void; children: ReactNode }) {
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id });
  return (
    <div ref={setNodeRef} style={{ transform: CSS.Transform.toString(transform), transition }}>
      <Card id={id} title={title} editMode={editMode} onRemove={onRemove} handleProps={{ ...attributes, ...listeners }}>
        {children}
      </Card>
    </div>
  );
}

export default function Dashboard({ notes, stats, layout, editMode, onSelectNote, onChangeLayout, onToggleEdit }: Props) {
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));
  const ctx = { notes, stats, onSelectNote };
  const active = layout.filter(k => WIDGETS[k]);
  const available = WIDGET_KEYS.filter(k => !active.includes(k));
  const remove = (id: string) => onChangeLayout(active.filter(k => k !== id));
  const onDragEnd = (e: DragEndEvent) => {
    const { active: a, over } = e;
    if (over && a.id !== over.id) {
      onChangeLayout(arrayMove(active, active.indexOf(String(a.id)), active.indexOf(String(over.id))));
    }
  };

  return (
    <div className="h-full overflow-y-auto p-6" style={{ background: '#fef9c3' }}>
      <div className="flex items-center justify-between mb-4 flex-wrap gap-2">
        <h1 className="text-2xl font-bold text-gray-900">Dashboard</h1>
        <div className="flex items-center gap-2 flex-wrap">
          {editMode && available.map(k => (
            <button key={k} onClick={() => onChangeLayout([...active, k])} className="px-2 py-1 rounded text-xs bg-white border border-gray-300 hover:bg-gray-100">+ {WIDGETS[k].label}</button>
          ))}
          <button onClick={onToggleEdit} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: editMode ? '#fde047' : 'white', border: '1px solid #e7d27a', color: '#1c1917' }}>{editMode ? 'Fertig' : 'Bearbeiten'}</button>
        </div>
      </div>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={active} strategy={verticalListSortingStrategy}>
          <div className="grid grid-cols-2 gap-4">
            {active.map(k => (
              <SortableCard key={k} id={k} title={WIDGETS[k].label} editMode={editMode} onRemove={remove}>
                {WIDGETS[k].render(ctx)}
              </SortableCard>
            ))}
          </div>
        </SortableContext>
      </DndContext>
      {active.length === 0 && <p className="text-sm text-gray-500">Keine Widgets. Klicke „Bearbeiten" und füge welche hinzu.</p>}
    </div>
  );
}
