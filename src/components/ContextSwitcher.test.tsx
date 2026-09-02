import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import i18n from '../i18n';
import ContextSwitcher from './ContextSwitcher';
import { OPEN_CONTEXTS_EVENT } from '../shortcuts';

const { serverAuthBegin, openExternal } = vi.hoisted(() => ({
  serverAuthBegin: vi.fn().mockResolvedValue('https://srv.example/oauth/authorize?state=x'),
  openExternal: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../api', () => ({
  api: {
    contexts: {
      list: vi.fn().mockResolvedValue([
        { id: 'a', label: '', kind: 'local', path: '/a.db', serverUrl: '', active: true },
        { id: 'b', label: 'Arbeit', kind: 'local', path: '/b.db', serverUrl: '', active: false },
        { id: 's', label: 'notes.example', kind: 'server', path: '/s.db', serverUrl: 'https://notes.example', active: false },
      ]),
      switch: vi.fn().mockResolvedValue(undefined),
      add: vi.fn().mockResolvedValue(undefined),
      serverAuthBegin,
      syncStatus: vi.fn().mockResolvedValue({ state: 'local', lastSyncedAt: 0, pending: 0 }),
    },
    openExternal,
    onContextChanged: () => () => {},
    onSyncStatus: () => () => {},
  },
}));

const switchLabel = i18n.t('contexts.switch');

beforeEach(() => {
  vi.clearAllMocks();
});

describe('ContextSwitcher', () => {
  it('lists local and server contexts on open', async () => {
    render(<ContextSwitcher />);
    const trigger = await screen.findByLabelText(switchLabel);
    await waitFor(() => expect(trigger).toBeInTheDocument());
    fireEvent.click(trigger);
    expect(await screen.findByText('Arbeit')).toBeInTheDocument();
    expect(await screen.findByText('notes.example')).toBeInTheDocument();
  });

  it('starts the browser auth flow when adding a server', async () => {
    render(<ContextSwitcher />);
    fireEvent.click(await screen.findByLabelText(switchLabel));
    fireEvent.click(await screen.findByText(i18n.t('contexts.addServer')));
    const input = await screen.findByPlaceholderText(i18n.t('contexts.addServerPrompt'));
    fireEvent.change(input, { target: { value: 'https://notes.example' } });
    fireEvent.click(screen.getByRole('button', { name: i18n.t('contexts.addServer') }));
    await waitFor(() => expect(serverAuthBegin).toHaveBeenCalledWith('https://notes.example'));
    await waitFor(() => expect(openExternal).toHaveBeenCalledWith('https://srv.example/oauth/authorize?state=x'));
  });

  it('switches the active context when a non-active entry is clicked', async () => {
    const { api } = await import('../api');
    render(<ContextSwitcher />);
    fireEvent.click(await screen.findByLabelText(switchLabel));
    fireEvent.click(await screen.findByText('Arbeit'));
    expect(api.contexts.switch).toHaveBeenCalledWith('b');
  });

  it('does not switch when clicking the already-active context', async () => {
    const { api } = await import('../api');
    render(<ContextSwitcher />);
    fireEvent.click(await screen.findByLabelText(switchLabel));
    // "Lokal" appears twice: once as the trigger's current label, once as the
    // active entry in the opened menu — target the menu item specifically.
    const entries = await screen.findAllByText('Lokal');
    fireEvent.click(entries[entries.length - 1]);
    expect(api.contexts.switch).not.toHaveBeenCalled();
  });

  it('shows an error and stops connecting if starting server auth throws', async () => {
    serverAuthBegin.mockRejectedValueOnce(new Error('offline'));
    render(<ContextSwitcher />);
    fireEvent.click(await screen.findByLabelText(switchLabel));
    fireEvent.click(await screen.findByText(i18n.t('contexts.addServer')));
    fireEvent.change(await screen.findByPlaceholderText(i18n.t('contexts.addServerPrompt')), { target: { value: 'https://bad.example' } });
    fireEvent.click(screen.getByRole('button', { name: i18n.t('contexts.addServer') }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Verbindung fehlgeschlagen');
  });

  it('shows the manage item and calls onManage when provided', async () => {
    const onManage = vi.fn();
    render(<ContextSwitcher onManage={onManage} />);
    fireEvent.click(await screen.findByLabelText(switchLabel));
    fireEvent.click(await screen.findByText(i18n.t('contexts.manage')));
    expect(onManage).toHaveBeenCalledOnce();
  });

  it('omits the manage item when onManage is not provided', async () => {
    render(<ContextSwitcher />);
    fireEvent.click(await screen.findByLabelText(switchLabel));
    await screen.findByText(i18n.t('contexts.add'));
    expect(screen.queryByText(i18n.t('contexts.manage'))).not.toBeInTheDocument();
  });

  it('applies the larger mobile touch-target classes when mobile is set', async () => {
    render(<ContextSwitcher mobile />);
    const trigger = await screen.findByLabelText(switchLabel);
    expect(trigger.className).toContain('px-3');
    expect(trigger.className).toContain('py-2.5');
    expect(trigger.className).toContain('text-sm');
  });

  it('uses the compact classes on desktop by default', async () => {
    render(<ContextSwitcher />);
    const trigger = await screen.findByLabelText(switchLabel);
    expect(trigger.className).toContain('px-2');
    expect(trigger.className).toContain('py-1.5');
    expect(trigger.className).toContain('text-xs');
  });

  it('adds a new local context and closes the prompt', async () => {
    const { api } = await import('../api');
    render(<ContextSwitcher />);
    fireEvent.click(await screen.findByLabelText(switchLabel));
    fireEvent.click(await screen.findByText(i18n.t('contexts.add')));
    const input = await screen.findByPlaceholderText(i18n.t('contexts.addPrompt'));
    fireEvent.change(input, { target: { value: 'Neuer Kontext' } });
    fireEvent.click(screen.getByRole('button', { name: i18n.t('contexts.add') }));
    expect(api.contexts.add).toHaveBeenCalledWith('Neuer Kontext');
    expect(screen.queryByPlaceholderText(i18n.t('contexts.addPrompt'))).not.toBeInTheDocument();
  });

  it('cancels adding a local context without calling the bridge', async () => {
    const { api } = await import('../api');
    render(<ContextSwitcher />);
    fireEvent.click(await screen.findByLabelText(switchLabel));
    fireEvent.click(await screen.findByText(i18n.t('contexts.add')));
    const input = await screen.findByPlaceholderText(i18n.t('contexts.addPrompt'));
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(screen.queryByPlaceholderText(i18n.t('contexts.addPrompt'))).not.toBeInTheDocument();
    expect(api.contexts.add).not.toHaveBeenCalled();
  });

  it('opens the menu, anchored to the trigger button, when the context-picker hotkey fires', async () => {
    render(<ContextSwitcher />);
    await screen.findByLabelText(switchLabel);
    expect(screen.queryByText(i18n.t('contexts.add'))).not.toBeInTheDocument();
    fireEvent(window, new Event(OPEN_CONTEXTS_EVENT));
    expect(await screen.findByText(i18n.t('contexts.add'))).toBeInTheDocument();
  });
});
