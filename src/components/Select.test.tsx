import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Select from './Select';
describe('Select', () => {
  it('shows the selected option label', () => {
    render(<Select value="b" options={[{ value: 'a', label: 'Alpha' }, { value: 'b', label: 'Beta' }]} onChange={vi.fn()} />);
    expect(screen.getByText('Beta')).toBeInTheDocument();
  });
});
