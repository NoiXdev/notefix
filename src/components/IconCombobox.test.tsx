import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import IconCombobox from './IconCombobox';

describe('IconCombobox', () => {
  it('filters by query and picks fa:<name>', () => {
    const onPick = vi.fn();
    render(<IconCombobox value="" onPick={onPick} />);
    fireEvent.change(screen.getByPlaceholderText('Icon suchen…'), { target: { value: 'star' } });
    fireEvent.click(screen.getByText('star'));
    expect(onPick).toHaveBeenCalledWith('fa:star');
  });
});
