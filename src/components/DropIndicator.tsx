/** Insertion line shown while dragging a note/folder. Indented to the target's
 *  depth so the nesting level of the drop is obvious, with a dot at the left end
 *  marking the exact insertion point. */
export default function DropIndicator({ mode, indent }: { mode: 'before' | 'after'; indent: number }) {
  const edge = mode === 'before' ? { top: -1.5 } : { bottom: -1.5 };
  return (
    <div className="pointer-events-none absolute z-20" style={{ left: indent, right: 6, ...edge }} aria-hidden>
      <div className="relative h-[3px] rounded-full" style={{ background: '#facc15' }}>
        <div
          className="absolute h-2 w-2 rounded-full"
          style={{ left: -3, top: -2.5, background: '#facc15', boxShadow: '0 0 0 2px rgba(250, 204, 21, 0.3)' }}
        />
      </div>
    </div>
  );
}
