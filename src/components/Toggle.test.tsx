import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Toggle from './Toggle';
describe('Toggle', () => {
  it('reflects checked and calls onChange', () => {
    const onChange = vi.fn();
    render(<Toggle checked onChange={onChange} label="Test" />);
    const sw = screen.getByLabelText('Test');
    expect(sw).toHaveAttribute('aria-checked', 'true');
    fireEvent.click(sw);
    expect(onChange).toHaveBeenCalledOnce();
  });
});
