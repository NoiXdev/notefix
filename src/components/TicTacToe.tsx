import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { type Board, winner, isFull, aiMove } from '../ticTacToe';
interface Props { onClose: () => void; }
const empty = (): Board => Array(9).fill(null);
export default function TicTacToe({ onClose }: Props) {
  const { t } = useTranslation();
  const [board, setBoard] = useState<Board>(empty);
  const w = winner(board);
  const over = w !== null || isFull(board);
  const status = w === 'X' ? t('game.won') : w === 'O' ? t('game.lost') : isFull(board) ? t('game.draw') : t('game.youAreX');
  const play = (i: number) => {
    if (board[i] || over) return;
    const next = board.slice();
    next[i] = 'X';
    if (!winner(next) && !isFull(next)) {
      const ai = aiMove(next);
      if (ai >= 0) next[ai] = 'O';
    }
    setBoard(next);
  };
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onClose}>
      <div className="rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <div className="flex items-center justify-between mb-3 gap-6">
          <h2 className="text-gray-100 text-sm font-semibold">{t('game.title')}</h2>
          <span className="text-xs text-gray-400">{status}</span>
        </div>
        <div className="grid grid-cols-3 gap-1">
          {board.map((c, i) => (
            <button key={i} onClick={() => play(i)} aria-label={t('game.cell', { index: i })} className="w-16 h-16 rounded bg-gray-800 hover:bg-gray-700 text-2xl font-bold flex items-center justify-center" style={{ color: c === 'X' ? '#fde047' : '#f87171' }}>
              {c}
            </button>
          ))}
        </div>
        <div className="flex justify-end gap-2 mt-4">
          <button onClick={() => setBoard(empty())} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">{t('game.reset')}</button>
          <button onClick={onClose} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: '#fde047', color: '#1c1917' }}>{t('game.close')}</button>
        </div>
      </div>
    </div>
  );
}
