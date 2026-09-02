import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Select from './Select';

const options = [{ value: 'a', label: 'Alpha' }, { value: 'b', label: 'Beta' }];

describe('Select', () => {
  it('shows the selected option label', () => {
    render(<Select value="b" options={options} onChange={vi.fn()} />);
    expect(screen.getByText('Beta')).toBeInTheDocument();
  });

  it('shows a blank control when the value matches no option', () => {
    render(<Select value="missing" options={options} onChange={vi.fn()} />);
    expect(screen.queryByText('Alpha')).not.toBeInTheDocument();
    expect(screen.queryByText('Beta')).not.toBeInTheDocument();
  });

  it('opens the menu and calls onChange with the picked value', () => {
    const onChange = vi.fn();
    const { container } = render(<Select value="a" options={options} onChange={onChange} />);
    const control = container.querySelector('input')!;
    fireEvent.focus(control);
    fireEvent.keyDown(control, { key: 'ArrowDown' });
    fireEvent.click(screen.getByText('Beta'));
    expect(onChange).toHaveBeenCalledWith('b');
  });

  it('renders every option once the menu is open', () => {
    const { container } = render(<Select value="a" options={options} onChange={vi.fn()} />);
    const control = container.querySelector('input')!;
    fireEvent.focus(control);
    fireEvent.keyDown(control, { key: 'ArrowDown' });
    expect(screen.getAllByText('Alpha').length).toBeGreaterThan(0);
    expect(screen.getByText('Beta')).toBeInTheDocument();
  });
});
