export type ShortcutContext = 'main' | 'window';
export interface ShortcutAction { id: string; labelKey: string; defaultCombo: string; context: ShortcutContext; }

export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  { id: 'navPrev', labelKey: 'shortcuts.actions.navPrev', defaultCombo: 'ArrowUp', context: 'main' },
  { id: 'navNext', labelKey: 'shortcuts.actions.navNext', defaultCombo: 'ArrowDown', context: 'main' },
  { id: 'newNote', labelKey: 'shortcuts.actions.newNote', defaultCombo: 'Mod+N', context: 'main' },
  { id: 'newFolder', labelKey: 'shortcuts.actions.newFolder', defaultCombo: 'Mod+Shift+N', context: 'main' },
  { id: 'archive', labelKey: 'shortcuts.actions.archive', defaultCombo: 'Mod+E', context: 'main' },
  { id: 'switchContextNext', labelKey: 'shortcuts.actions.switchContextNext', defaultCombo: 'Mod+Shift+K', context: 'main' },
  { id: 'openContextPicker', labelKey: 'shortcuts.actions.openContextPicker', defaultCombo: 'Mod+K', context: 'main' },
  { id: 'openSearch', labelKey: 'shortcuts.actions.openSearch', defaultCombo: 'Mod+P', context: 'main' },
  { id: 'findInNote', labelKey: 'shortcuts.actions.findInNote', defaultCombo: 'Mod+F', context: 'main' },
  { id: 'closeWindow', labelKey: 'shortcuts.actions.closeWindow', defaultCombo: 'Escape', context: 'window' },
];

/** Window event the context-picker hotkey dispatches; ContextSwitcher opens on it. */
export const OPEN_CONTEXTS_EVENT = 'notefix:open-contexts';

const ACTION_IDS = new Set(SHORTCUT_ACTIONS.map(a => a.id));

function normalizeKey(key: string): string {
  return key.length === 1 ? key.toUpperCase() : key;
}

export function eventToCombo(e: KeyboardEvent): string | null {
  if (e.key === 'Control' || e.key === 'Meta' || e.key === 'Shift' || e.key === 'Alt') return null;
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push('Mod');
  if (e.shiftKey) parts.push('Shift');
  if (e.altKey) parts.push('Alt');
  parts.push(normalizeKey(e.key));
  return parts.join('+');
}

export function matchesCombo(e: KeyboardEvent, combo: string): boolean {
  return eventToCombo(e) === combo;
}

export function comboLabel(combo: string): string {
  return combo
    .split('+')
    .map(p => (p === 'ArrowUp' ? '↑' : p === 'ArrowDown' ? '↓' : p === 'ArrowLeft' ? '←' : p === 'ArrowRight' ? '→' : p))
    .join(' + ');
}

export function resolveBindings(overrides: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const a of SHORTCUT_ACTIONS) out[a.id] = overrides[a.id] ?? a.defaultCombo;
  return out;
}

export function parseShortcuts(raw: string | undefined): Record<string, string> {
  if (!raw) return {};
  try {
    const v = JSON.parse(raw);
    if (!v || typeof v !== 'object' || Array.isArray(v)) return {};
    const out: Record<string, string> = {};
    for (const [k, val] of Object.entries(v)) if (ACTION_IDS.has(k) && typeof val === 'string') out[k] = val;
    return out;
  } catch {
    return {};
  }
}
