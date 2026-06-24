export type ShortcutContext = 'main' | 'window';
export interface ShortcutAction { id: string; label: string; defaultCombo: string; context: ShortcutContext; }

export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  { id: 'navPrev', label: 'Vorherige Notiz', defaultCombo: 'ArrowUp', context: 'main' },
  { id: 'navNext', label: 'Nächste Notiz', defaultCombo: 'ArrowDown', context: 'main' },
  { id: 'newNote', label: 'Neue Notiz', defaultCombo: 'Mod+N', context: 'main' },
  { id: 'newFolder', label: 'Neuer Ordner', defaultCombo: 'Mod+Shift+N', context: 'main' },
  { id: 'archive', label: 'Notiz archivieren / wiederherstellen', defaultCombo: 'Mod+E', context: 'main' },
  { id: 'closeWindow', label: 'Losgelöstes Fenster schließen', defaultCombo: 'Escape', context: 'window' },
];

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
