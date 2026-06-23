import { describe, it, expect } from 'vitest';
import { winner, aiMove, type Board } from './ticTacToe';
describe('ticTacToe', () => {
  it('detects a winning row', () => {
    expect(winner(['X', 'X', 'X', null, null, null, null, null, null])).toBe('X');
  });
  it('aiMove takes a winning move for O', () => {
    const b: Board = ['O', 'O', null, 'X', 'X', null, null, null, null];
    expect(aiMove(b)).toBe(2);
  });
  it('aiMove blocks an imminent X row', () => {
    const b: Board = ['X', 'X', null, null, 'O', null, null, null, null];
    expect(aiMove(b)).toBe(2);
  });
});
