import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import WorkspacePicker from './WorkspacePicker';

const { mockServerWorkspaces, mockBindWorkspace } = vi.hoisted(() => ({
  mockServerWorkspaces: vi.fn(),
  mockBindWorkspace: vi.fn(),
}));

vi.mock('../api', () => ({
  api: {
    contexts: {
      serverWorkspaces: mockServerWorkspaces,
      bindWorkspace: mockBindWorkspace,
    },
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  mockBindWorkspace.mockResolvedValue([]);
});

describe('WorkspacePicker', () => {
  it('shows a loading state, then lists workspaces with their role', async () => {
    mockServerWorkspaces.mockResolvedValue([
      { id: 'w1', name: 'Team A', role: 'owner' },
      { id: 'w2', name: 'Team B', role: 'member' },
    ]);
    render(<WorkspacePicker contextId="ctx-1" onClose={vi.fn()} />);
    expect(screen.getByText('Lade…')).toBeInTheDocument();
    expect(await screen.findByText('Team A')).toBeInTheDocument();
    expect(screen.getByText('Team B')).toBeInTheDocument();
    expect(screen.getByText('· owner')).toBeInTheDocument();
    expect(screen.getByText('· member')).toBeInTheDocument();
  });

  it('binds the selected workspace and closes on selection', async () => {
    mockServerWorkspaces.mockResolvedValue([{ id: 'w1', name: 'Team A', role: 'owner' }]);
    const onClose = vi.fn();
    render(<WorkspacePicker contextId="ctx-1" onClose={onClose} />);
    fireEvent.click(await screen.findByText('Team A'));
    await waitFor(() => expect(mockBindWorkspace).toHaveBeenCalledWith('ctx-1', 'w1', 'Team A'));
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it('shows an error message when loading workspaces fails', async () => {
    mockServerWorkspaces.mockRejectedValue(new Error('network down'));
    render(<WorkspacePicker contextId="ctx-1" onClose={vi.fn()} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Workspaces konnten nicht geladen werden');
  });

  it('calls onClose when the backdrop is clicked', async () => {
    mockServerWorkspaces.mockResolvedValue([]);
    const onClose = vi.fn();
    render(<WorkspacePicker contextId="ctx-1" onClose={onClose} />);
    expect(await screen.findByText('Workspace wählen')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Workspace wählen').closest('.fixed')!);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does not close when clicking inside the dialog panel', async () => {
    mockServerWorkspaces.mockResolvedValue([]);
    const onClose = vi.fn();
    render(<WorkspacePicker contextId="ctx-1" onClose={onClose} />);
    fireEvent.click(await screen.findByText('Workspace wählen'));
    expect(onClose).not.toHaveBeenCalled();
  });
});
