import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import Logo from './Logo';

describe('Logo', () => {
  it('renders the Notefix logo image', () => {
    render(<Logo />);
    expect(screen.getByAltText('Notefix')).toBeInTheDocument();
  });
});
