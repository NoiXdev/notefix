import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import i18n from '../i18n';
import ContextSwitcher from './ContextSwitcher';

vi.mock('../api', () => ({
  api: {
    contexts: {
      list: vi.fn().mockResolvedValue([
        { id: 'a', label: '', kind: 'local', path: '/a.db', active: true },
        { id: 'b', label: 'Arbeit', kind: 'local', path: '/b.db', active: false },
      ]),
      switch: vi.fn().mockResolvedValue(undefined),
    },
    onContextChanged: () => () => {},
  },
}));

// The `contexts.*` i18n keys land in a later task; until then t() returns the
// raw key. Derive the expected label from i18n so this test is robust either way.
const switchLabel = i18n.t('contexts.switch');

describe('ContextSwitcher', () => {
  it('shows the active context and lists others on open', async () => {
    render(<ContextSwitcher />);
    const trigger = await screen.findByLabelText(switchLabel);
    await waitFor(() => expect(trigger).toBeInTheDocument());
    fireEvent.click(trigger);
    expect(await screen.findByText('Arbeit')).toBeInTheDocument();
  });
});
