import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import NoteList from './NoteList';
import type { Note } from '../types';

const note = (id: string, content: string, updatedAt = Date.now(), pinned = false): Note => ({ id, content, updatedAt, pinned });

const defaultProps = {
  notes: [],
  selectedId: null,
  onSelect: vi.fn(),
  onCreate: vi.fn(),
  onDelete: vi.fn(),
  onOpenSettings: vi.fn(),
  displayMode: 'flat' as const,
  onTogglePin: vi.fn(),
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

describe("NoteList — pinning", () => {
  it("flat mode shows a divider between pinned and unpinned", () => {
    const notes = [note('p', '<p>Pinned</p>', 2000, true), note('u', '<p>Unpinned</p>', 1000, false)];
    render(<NoteList {...defaultProps} notes={notes} />);
    expect(screen.getByTestId('pin-divider')).toBeInTheDocument();
  });

  it("sections mode shows group headers", () => {
    const notes = [note('p', '<p>Pinned</p>', 2000, true), note('u', '<p>Unpinned</p>', 1000, false)];
    render(<NoteList {...defaultProps} notes={notes} displayMode="sections" />);
    expect(screen.getByText('Angepinnt')).toBeInTheDocument();
    expect(screen.getByText('Weitere')).toBeInTheDocument();
  });

  it("right-click opens the menu and Anpinnen calls onTogglePin", () => {
    const onTogglePin = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>', 1000, false)]} onTogglePin={onTogglePin} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Anpinnen'));
    expect(onTogglePin).toHaveBeenCalledWith('a', true);
  });

  it("right-click on a pinned note offers Lösen", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>', 1000, true)]} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    expect(screen.getByText('Lösen')).toBeInTheDocument();
  });
});
