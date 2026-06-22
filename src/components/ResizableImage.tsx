import { useRef } from 'react';
import Image from '@tiptap/extension-image';
import {
  NodeViewWrapper,
  ReactNodeViewRenderer,
  type NodeViewProps,
} from '@tiptap/react';

const MIN_WIDTH = 60;

function ImageNodeView({ node, updateAttributes, selected, editor }: NodeViewProps) {
  const imgRef = useRef<HTMLImageElement>(null);

  const startResize = (e: React.PointerEvent) => {
    if (!editor.isEditable) return;
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startWidth = imgRef.current?.getBoundingClientRect().width ?? 0;

    const onMove = (ev: PointerEvent) => {
      const delta = ev.clientX - startX;
      const next = Math.max(MIN_WIDTH, Math.round(startWidth + delta));
      updateAttributes({ width: next });
    };
    const onUp = () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  };

  const width = node.attrs.width as number | string | null;
  const widthStyle = width == null ? undefined : typeof width === 'number' ? `${width}px` : width;

  return (
    <NodeViewWrapper
      className={`image-wrapper${selected ? ' is-selected' : ''}`}
      style={widthStyle ? { width: widthStyle } : undefined}
    >
      <img
        ref={imgRef}
        src={node.attrs.src}
        alt={node.attrs.alt ?? ''}
        title={node.attrs.title ?? ''}
        draggable={false}
      />
      {editor.isEditable && (
        <span
          className="image-resize-handle"
          onPointerDown={startResize}
          aria-hidden
        />
      )}
    </NodeViewWrapper>
  );
}

export const ResizableImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      width: {
        default: null,
        parseHTML: element => {
          const w = element.getAttribute('width');
          if (w) return Number.isFinite(Number(w)) ? Number(w) : w;
          const styleW = (element as HTMLElement).style?.width;
          if (styleW) {
            const px = parseInt(styleW, 10);
            return Number.isFinite(px) ? px : styleW;
          }
          return null;
        },
        renderHTML: attrs => {
          if (attrs.width == null) return {};
          return { width: String(attrs.width) };
        },
      },
    };
  },

  addNodeView() {
    return ReactNodeViewRenderer(ImageNodeView);
  },
});
