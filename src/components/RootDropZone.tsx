import { useDroppable } from '@dnd-kit/core';

export default function RootDropZone() {
  const { setNodeRef } = useDroppable({ id: 'root:into' });
  return <div ref={setNodeRef} className="h-16" aria-hidden />;
}
