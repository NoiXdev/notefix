export function mdCursor(text: string, selStart: number): { ln: number; col: number; length: number; lines: number } {
  const pos = Math.max(0, Math.min(selStart, text.length));
  const before = text.slice(0, pos);
  const ln = before.split('\n').length;
  const lastNl = before.lastIndexOf('\n');
  const col = before.length - lastNl; // 1-based (lastNl = -1 when on line 1)
  return { ln, col, length: text.length, lines: text.split('\n').length };
}

export function richCounts(text: string, selLen: number): { words: number; chars: number; sel: number } {
  const trimmed = text.trim();
  return { words: trimmed ? trimmed.split(/\s+/).length : 0, chars: text.length, sel: Math.max(0, selLen) };
}
