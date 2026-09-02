import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import VaultRotationRedeemDialog from './VaultRotationRedeemDialog';

function renderDialog() {
  const redeem = vi.fn<(code: string, passphrase: string) => Promise<void>>().mockResolvedValue(undefined);
  const onSuccess = vi.fn();
  const onCancel = vi.fn();
  render(<VaultRotationRedeemDialog redeem={redeem} onSuccess={onSuccess} onCancel={onCancel} />);
  return { redeem, onSuccess, onCancel };
}

describe('VaultRotationRedeemDialog', () => {
  it('redeems the code with the member’s own passphrase', async () => {
    const { redeem, onSuccess } = renderDialog();
    fireEvent.change(screen.getByPlaceholderText('Wechsel-Code'), { target: { value: 'AAAA-BBBB' } });
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'member-pw' } });
    fireEvent.click(screen.getByRole('button', { name: 'Schlüssel wechseln' }));

    await waitFor(() => expect(redeem).toHaveBeenCalledWith('AAAA-BBBB', 'member-pw'));
    await waitFor(() => expect(onSuccess).toHaveBeenCalledOnce());
  });

  it('needs both fields before it can be submitted', () => {
    renderDialog();
    const submit = screen.getByRole('button', { name: 'Schlüssel wechseln' });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByPlaceholderText('Wechsel-Code'), { target: { value: 'AAAA-BBBB' } });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'pw' } });
    expect(submit).toBeEnabled();
  });

  it('tells a rejected code and a wrong passphrase apart', async () => {
    const { redeem, onSuccess } = renderDialog();
    redeem.mockRejectedValueOnce(new Error('invalid rotation code'));
    fireEvent.change(screen.getByPlaceholderText('Wechsel-Code'), { target: { value: 'nope' } });
    const pass = screen.getByPlaceholderText('Passwort');
    fireEvent.change(pass, { target: { value: 'member-pw' } });
    fireEvent.keyDown(pass, { key: 'Enter' });
    expect(await screen.findByRole('alert')).toHaveTextContent('Code ist ungültig oder schon eingelöst');

    redeem.mockRejectedValueOnce(new Error('wrong passphrase'));
    fireEvent.keyDown(pass, { key: 'Enter' });
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Falsches Passwort'));
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it('says the vault has to be unlocked instead of blaming the code', async () => {
    const { redeem, onSuccess } = renderDialog();
    redeem.mockRejectedValueOnce(new Error('vault locked'));
    fireEvent.change(screen.getByPlaceholderText('Wechsel-Code'), { target: { value: 'AAAA-BBBB' } });
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'member-pw' } });
    fireEvent.click(screen.getByRole('button', { name: 'Schlüssel wechseln' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Entsperre zuerst den Tresor');
    expect(screen.queryByText(/Code ist ungültig/)).not.toBeInTheDocument();
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it('can be postponed', () => {
    const { onCancel, redeem } = renderDialog();
    fireEvent.click(screen.getByRole('button', { name: 'Später' }));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(redeem).not.toHaveBeenCalled();
  });

  it('closes on Escape', () => {
    const { onCancel } = renderDialog();
    fireEvent.keyDown(screen.getByPlaceholderText('Wechsel-Code'), { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
