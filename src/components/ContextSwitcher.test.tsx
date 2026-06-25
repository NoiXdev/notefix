import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import i18n from '../i18n';
import ContextSwitcher from './ContextSwitcher';

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
      serverAuthBegin,
    },
    openExternal,
    onContextChanged: () => () => {},
  },
}));

const switchLabel = i18n.t('contexts.switch');

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
});
