export interface Shortcut { keys: string; description: string; }
export const SHORTCUTS: Shortcut[] = [
  { keys: '↑ / ↓', description: 'Vorherige / nächste Notiz' },
  { keys: 'Mod + N', description: 'Neue Notiz' },
  { keys: 'Mod + Shift + N', description: 'Neuer Ordner' },
  { keys: 'Mod + E', description: 'Notiz archivieren / wiederherstellen' },
];
