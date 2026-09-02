import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SyncStatus from './SyncStatus';
import type { SyncStatus as S } from '../syncStatus';
import type { ContextInfo } from '../contexts';

const { mockSyncStatus, mockSyncNow, mockOnSyncStatus, mockOnContextChanged, mockList } = vi.hoisted(() => ({
  mockSyncStatus: vi.fn<() => Promise<S>>(),
  mockSyncNow: vi.fn(),
  mockOnSyncStatus: vi.fn<(cb: (s: S) => void) => () => void>(() => () => {}),
  mockOnContextChanged: vi.fn<(cb: () => void) => () => void>(() => () => {}),
  mockList: vi.fn<() => Promise<ContextInfo[]>>(),
}));

vi.mock('../api', () => ({
  api: {
    contexts: { syncStatus: mockSyncStatus, syncNow: mockSyncNow, list: mockList },
    onSyncStatus: mockOnSyncStatus,
    onContextChanged: mockOnContextChanged,
  },
}));

/** The active server context, by default with no rotation outstanding. */
const context = (overrides: Partial<ContextInfo> = {}): ContextInfo => ({
  id: 'srv', label: 'Server', kind: 'server', path: '', serverUrl: 'https://s', workspaceId: 'ws1',
  active: true, vaultExists: true, vaultBiometric: false, vaultGeneration: 1,
  vaultRotationPending: false, ...overrides,
});

beforeEach(() => {
  vi.clearAllMocks();
  mockOnSyncStatus.mockImplementation(() => () => {});
  mockOnContextChanged.mockImplementation(() => () => {});
  mockList.mockResolvedValue([context()]);
});

const status = (overrides: Partial<S> = {}): S => ({ state: 'synced', lastSyncedAt: 0, pending: 0, ...overrides });

describe('SyncStatus', () => {
  it('renders nothing before the initial status resolves', () => {
    mockSyncStatus.mockReturnValue(new Promise(() => {}));
    const { container } = render(<SyncStatus />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing for the local state', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'local' }));
    const { container } = render(<SyncStatus />);
    await new Promise(r => setTimeout(r, 0));
    expect(container).toBeEmptyDOMElement();
  });

  it('shows "unbound" for the unbound state', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'unbound' }));
    render(<SyncStatus />);
    expect(await screen.findByText('Kein Workspace')).toBeInTheDocument();
  });

  it('shows "syncing" with a spin class for the syncing state', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'syncing' }));
    render(<SyncStatus />);
    expect(await screen.findByText('Synchronisiere…')).toBeInTheDocument();
    expect(document.querySelector('.animate-spin')).toBeInTheDocument();
  });

  it('shows "offline" for the offline state', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'offline' }));
    render(<SyncStatus />);
    expect(await screen.findByText('Offline')).toBeInTheDocument();
  });

  it('shows "synced" for the synced state, with no spin class', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'synced' }));
    render(<SyncStatus />);
    expect(await screen.findByText('Synchronisiert')).toBeInTheDocument();
    expect(document.querySelector('.animate-spin')).not.toBeInTheDocument();
  });

  it('appends the pending count when greater than zero', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'synced', pending: 3 }));
    render(<SyncStatus />);
    expect(await screen.findByText('Synchronisiert · 3')).toBeInTheDocument();
  });

  it('calls syncNow when clicked', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'synced' }));
    render(<SyncStatus />);
    fireEvent.click(await screen.findByTitle('Jetzt synchronisieren'));
    expect(mockSyncNow).toHaveBeenCalledOnce();
  });

  it('subscribes to sync-status pushes and updates the displayed state', async () => {
    let pushed: ((s: S) => void) | undefined;
    mockOnSyncStatus.mockImplementation(cb => { pushed = cb; return () => {}; });
    mockSyncStatus.mockResolvedValue(status({ state: 'synced' }));
    render(<SyncStatus />);
    expect(await screen.findByText('Synchronisiert')).toBeInTheDocument();
    expect(pushed).toBeInstanceOf(Function);
    pushed?.(status({ state: 'offline' }));
    expect(await screen.findByText('Offline')).toBeInTheDocument();
  });

  it('refreshes the status when the active context changes', async () => {
    let changed: (() => void) | undefined;
    mockOnContextChanged.mockImplementation(cb => { changed = cb; return () => {}; });
    mockSyncStatus.mockResolvedValueOnce(status({ state: 'synced' }));
    render(<SyncStatus />);
    expect(await screen.findByText('Synchronisiert')).toBeInTheDocument();
    mockSyncStatus.mockResolvedValueOnce(status({ state: 'unbound' }));
    changed?.();
    expect(await screen.findByText('Kein Workspace')).toBeInTheDocument();
  });
  it('shows the pending key change of the active context', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'synced' }));
    mockList.mockResolvedValue([context({ vaultRotationPending: true })]);
    render(<SyncStatus />);
    expect(await screen.findByText('Schlüsselwechsel offen')).toBeInTheDocument();
  });

  it('ignores a pending key change on a context that is not active', async () => {
    mockSyncStatus.mockResolvedValue(status({ state: 'synced' }));
    mockList.mockResolvedValue([context({ active: false, vaultRotationPending: true })]);
    render(<SyncStatus />);
    expect(await screen.findByText('Synchronisiert')).toBeInTheDocument();
    expect(screen.queryByText('Schlüsselwechsel offen')).not.toBeInTheDocument();
  });
});
