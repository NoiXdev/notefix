import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';

// react-confetti draws to a <canvas> and measures the viewport — neither is
// meaningful in jsdom. Passthrough-mock (like react-grid-layout in
// Dashboard.test.tsx): render a stand-in that exposes the props we assert on
// and a button to fire onConfettiComplete on demand.
vi.mock('react-confetti', () => ({
  __esModule: true,
  default: (props: { width: number; height: number; confettiSource: { x: number; y: number }; onConfettiComplete: () => void }) => (
    <div
      data-testid="confetti"
      data-width={props.width}
      data-height={props.height}
      data-x={props.confettiSource.x}
      data-y={props.confettiSource.y}
    >
      <button onClick={() => props.onConfettiComplete()}>complete</button>
    </div>
  ),
}));

import ConfettiEasterEgg from './ConfettiEasterEgg';

describe('ConfettiEasterEgg', () => {
  it('renders nothing until a middle-click happens', () => {
    const { container } = render(<ConfettiEasterEgg />);
    expect(container).toBeEmptyDOMElement();
  });

  it('bursts confetti at the cursor position on a middle-mouse click', () => {
    render(<ConfettiEasterEgg />);
    fireEvent.mouseDown(window, { button: 1, clientX: 42, clientY: 17 });
    const burst = screen.getByTestId('confetti');
    expect(burst).toHaveAttribute('data-x', '42');
    expect(burst).toHaveAttribute('data-y', '17');
  });

  it('ignores non-middle mouse buttons', () => {
    const { container } = render(<ConfettiEasterEgg />);
    fireEvent.mouseDown(window, { button: 0, clientX: 42, clientY: 17 });
    expect(container).toBeEmptyDOMElement();
  });

  it('accumulates a second burst on a second middle-click without removing the first', () => {
    render(<ConfettiEasterEgg />);
    fireEvent.mouseDown(window, { button: 1, clientX: 1, clientY: 1 });
    fireEvent.mouseDown(window, { button: 1, clientX: 2, clientY: 2 });
    expect(screen.getAllByTestId('confetti')).toHaveLength(2);
  });

  it('removes a burst when its onConfettiComplete fires, leaving the others', () => {
    render(<ConfettiEasterEgg />);
    fireEvent.mouseDown(window, { button: 1, clientX: 1, clientY: 1 });
    fireEvent.mouseDown(window, { button: 1, clientX: 2, clientY: 2 });
    const [first] = screen.getAllByTestId('confetti');
    fireEvent.click(first.querySelector('button')!);
    expect(screen.getAllByTestId('confetti')).toHaveLength(1);
  });

  it('tracks window resize into the confetti dimensions', () => {
    render(<ConfettiEasterEgg />);
    Object.defineProperty(window, 'innerWidth', { value: 999, configurable: true });
    Object.defineProperty(window, 'innerHeight', { value: 888, configurable: true });
    fireEvent(window, new Event('resize'));
    fireEvent.mouseDown(window, { button: 1, clientX: 5, clientY: 5 });
    const burst = screen.getByTestId('confetti');
    expect(burst).toHaveAttribute('data-width', '999');
    expect(burst).toHaveAttribute('data-height', '888');
  });

  it('removes the mousedown and resize listeners on unmount', () => {
    const addSpy = vi.spyOn(window, 'addEventListener');
    const removeSpy = vi.spyOn(window, 'removeEventListener');
    const { unmount } = render(<ConfettiEasterEgg />);
    const registeredTypes = addSpy.mock.calls.map(c => c[0]);
    expect(registeredTypes).toEqual(expect.arrayContaining(['resize', 'mousedown']));
    unmount();
    const removedTypes = removeSpy.mock.calls.map(c => c[0]);
    expect(removedTypes).toEqual(expect.arrayContaining(['resize', 'mousedown']));
    addSpy.mockRestore();
    removeSpy.mockRestore();
  });
});
