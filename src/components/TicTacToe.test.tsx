import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import TicTacToe from './TicTacToe';
describe('TicTacToe', () => {
  it('places X on click and resets with Neu', () => {
    render(<TicTacToe onClose={vi.fn()} />);
    fireEvent.click(screen.getByLabelText('Feld 0'));
    expect(screen.getByLabelText('Feld 0')).toHaveTextContent('X');
    fireEvent.click(screen.getByText('Neu'));
    expect(screen.getByLabelText('Feld 0')).toHaveTextContent('');
  });
});
