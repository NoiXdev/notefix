import { fas } from '@fortawesome/free-solid-svg-icons';
import type { IconDefinition } from '@fortawesome/fontawesome-svg-core';

export const FA_BY_NAME: Record<string, IconDefinition> = {};
for (const def of Object.values(fas) as IconDefinition[]) {
  if (def && def.iconName) FA_BY_NAME[def.iconName] = def;
}
export const FA_NAMES = Object.keys(FA_BY_NAME);

const SUGGESTIONS = [
  'folder', 'star', 'briefcase', 'house', 'lightbulb', 'heart', 'book', 'code', 'music', 'camera',
  'flag', 'bell', 'bookmark', 'tag', 'list', 'calendar', 'envelope', 'user', 'graduation-cap',
  'cart-shopping', 'plane', 'gamepad', 'mug-hot', 'dumbbell', 'paw', 'leaf', 'fire', 'gift', 'moon', 'sun',
].filter(n => n in FA_BY_NAME);

/** Empty query → curated suggestions; otherwise substring match on icon names, capped. */
export function searchIcons(query: string, limit = 80): string[] {
  const q = query.trim().toLowerCase();
  if (!q) return SUGGESTIONS;
  const out: string[] = [];
  for (const name of FA_NAMES) {
    if (name.includes(q)) {
      out.push(name);
      if (out.length >= limit) break;
    }
  }
  return out;
}

export const EMOJIS = [
  '📁', '📂', '⭐', '💼', '🏠', '💡', '❤️', '📚', '💻', '🎵',
  '📷', '🚩', '🔔', '🔖', '🏷️', '📅', '✉️', '👤', '🎓', '🛒',
  '✈️', '🎮', '☕', '🏋️', '🐾', '🍃', '🔥', '🎁', '🌙', '☀️',
];
