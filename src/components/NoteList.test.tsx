import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import NoteList from './NoteList';
import type { Note } from '../types';

const note = (id: string, content: string, updatedAt = Date.now()): Note => ({ id, content, updatedAt });

const defaultProps = {
  notes: [],
  selectedId: null,
  onSelect: vi.fn(),
  onCreate: vi.fn(),
  onDelete: vi.fn(),
};

beforeEach(() => vi.clearAllMocks());

describe('NoteList — empty state', () => {
  it('shows the empty state message when there are no notes', () => {
    render(<NoteList {...defaultProps} />);
    expect(screen.getByText(/no notes yet/i)).toBeInTheDocument();
  });

  it('does not render any note buttons when empty', () => {
    render(<NoteList {...defaultProps} />);
    expect(screen.queryByTitle('Delete note')).not.toBeInTheDocument();
  });
});

describe('NoteList — rendering notes', () => {
  it('renders stripped plain-text preview from HTML content', () => {
    render(<NoteList {...defaultProps} notes={[note('1', '<b>Buy milk</b>')]} />);
    expect(screen.getByText('Buy milk')).toBeInTheDocument();
  });

  it('falls back to "New note" for empty content', () => {
    render(<NoteList {...defaultProps} notes={[note('1', '')]} />);
    expect(screen.getByText('New note')).toBeInTheDocument();
  });

  it('truncates content preview to 60 characters', () => {
    const long = 'A'.repeat(80);
    render(<NoteList {...defaultProps} notes={[note('1', `<p>${long}</p>`)]} />);
    expect(screen.getByText('A'.repeat(60))).toBeInTheDocument();
  });

  it('renders one row per note', () => {
    const notes = [note('1', '<p>First</p>'), note('2', '<p>Second</p>'), note('3', '<p>Third</p>')];
    render(<NoteList {...defaultProps} notes={notes} />);
    expect(screen.getAllByTitle('Delete note')).toHaveLength(3);
  });

  it('applies selected style to the active note', () => {
    const notes = [note('1', '<p>Alpha</p>'), note('2', '<p>Beta</p>')];
    render(<NoteList {...defaultProps} notes={notes} selectedId="1" />);

    const alphaBtn = screen.getByText('Alpha').closest('button')!;
    const betaBtn  = screen.getByText('Beta').closest('button')!;

    expect(alphaBtn).toHaveClass('bg-gray-800');
    expect(betaBtn).not.toHaveClass('bg-gray-800');
  });
});

describe('NoteList — interactions', () => {
  it('calls onCreate when the + button is clicked', () => {
    render(<NoteList {...defaultProps} />);
    fireEvent.click(screen.getByTitle('New note'));
    expect(defaultProps.onCreate).toHaveBeenCalledOnce();
  });

  it('calls onSelect with the note id when a note is clicked', () => {
    render(<NoteList {...defaultProps} notes={[note('42', '<p>Click me</p>')]} />);
    fireEvent.click(screen.getByText('Click me'));
    expect(defaultProps.onSelect).toHaveBeenCalledWith('42');
  });

  it('calls onDelete with the note id when the delete button is clicked', () => {
    render(<NoteList {...defaultProps} notes={[note('7', '<p>Delete me</p>')]} />);
    fireEvent.click(screen.getByTitle('Delete note'));
    expect(defaultProps.onDelete).toHaveBeenCalledWith('7');
  });

  it('does not call onSelect when the delete button is clicked', () => {
    render(<NoteList {...defaultProps} notes={[note('7', '<p>Note</p>')]} />);
    fireEvent.click(screen.getByTitle('Delete note'));
    expect(defaultProps.onSelect).not.toHaveBeenCalled();
  });
});
