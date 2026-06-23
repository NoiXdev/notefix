export type Cell = 'X' | 'O' | null;
export type Board = Cell[];
const LINES = [[0, 1, 2], [3, 4, 5], [6, 7, 8], [0, 3, 6], [1, 4, 7], [2, 5, 8], [0, 4, 8], [2, 4, 6]];
export function winner(b: Board): 'X' | 'O' | null {
  for (const [a, c, d] of LINES) {
    if (b[a] && b[a] === b[c] && b[a] === b[d]) return b[a] as 'X' | 'O';
  }
  return null;
}
export function isFull(b: Board): boolean {
  return b.every(c => c !== null);
}
export function aiMove(b: Board): number {
  const free = b.map((c, i) => (c === null ? i : -1)).filter(i => i >= 0);
  const tryFor = (mark: 'X' | 'O') => {
    for (const i of free) {
      const copy = b.slice();
      copy[i] = mark;
      if (winner(copy) === mark) return i;
    }
    return -1;
  };
  const win = tryFor('O');
  if (win >= 0) return win;
  const block = tryFor('X');
  if (block >= 0) return block;
  if (b[4] === null) return 4;
  return free[0] ?? -1;
}
