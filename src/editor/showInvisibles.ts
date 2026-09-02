import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { Node as PMNode } from '@tiptap/pm/model';

/** Renders formatting marks: a middot over each space and a ↵ for hard breaks.
 *  Paragraph marks (¶) are pure CSS via the `.show-invisibles` container class.
 *  Toggled with the `setInvisibles(enabled)` command. */
interface InvState {
  enabled: boolean;
  deco: DecorationSet;
}

export const invisiblesKey = new PluginKey<InvState>('show-invisibles');

/** Pure: 0-based [from, to) ranges of every space character in `text` — the
 *  positions that get the middot decoration when invisibles are shown. */
export function invisibleDecorationsFor(text: string): Array<[number, number]> {
  const ranges: Array<[number, number]> = [];
  for (let i = 0; i < text.length; i++) {
    if (text[i] === ' ') ranges.push([i, i + 1]);
  }
  return ranges;
}

function build(doc: PMNode, enabled: boolean): DecorationSet {
  if (!enabled) return DecorationSet.empty;
  const decos: Decoration[] = [];
  doc.descendants((node, pos) => {
    if (node.isText && node.text) {
      for (const [s, e] of invisibleDecorationsFor(node.text)) {
        decos.push(Decoration.inline(pos + s, pos + e, { class: 'inv-space' }));
      }
    } else if (node.type.name === 'hardBreak') {
      decos.push(
        Decoration.widget(pos, () => {
          const el = document.createElement('span');
          el.className = 'inv-break';
          el.textContent = '↵';
          return el;
        }, { side: -1 }),
      );
    }
    return true;
  });
  return DecorationSet.create(doc, decos);
}

export const ShowInvisibles = Extension.create({
  name: 'showInvisibles',

  addProseMirrorPlugins() {
    return [
      new Plugin<InvState>({
        key: invisiblesKey,
        state: {
          init: () => ({ enabled: false, deco: DecorationSet.empty }),
          apply(tr, prev, _old, newState) {
            const meta = tr.getMeta(invisiblesKey) as boolean | undefined;
            if (meta === undefined && !tr.docChanged) return prev;
            const enabled = meta ?? prev.enabled;
            return { enabled, deco: build(newState.doc, enabled) };
          },
        },
        props: {
          decorations(state) {
            return invisiblesKey.getState(state)?.deco;
          },
        },
      }),
    ];
  },

  addCommands() {
    return {
      setInvisibles:
        (enabled: boolean) =>
        ({ state, dispatch }) => {
          if (dispatch) dispatch(state.tr.setMeta(invisiblesKey, enabled));
          return true;
        },
    };
  },
});

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    showInvisibles: {
      setInvisibles: (enabled: boolean) => ReturnType;
    };
  }
}
