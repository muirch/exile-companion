import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import { SettingsApp } from './SettingsApp';

vi.mock('../ipc/commands', async (importOriginal) => {
  const real = await importOriginal<typeof commands>();
  const stubs = Object.fromEntries(
    Object.keys(real).map((name) => [name, vi.fn(() => Promise.resolve(null))]),
  );
  return {
    ...stubs,
    getHotkeys: vi.fn(() => new Promise(() => {})),
  };
});

describe('SettingsApp', () => {
  it('shows why a pasted build did not load', async () => {
    vi.mocked(commands.pasteBuild).mockRejectedValueOnce("Couldn't download the build: timeout");
    render(<SettingsApp />);
    await userEvent.click(screen.getByRole('button', { name: 'Paste code or link' }));
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't download the build: timeout");
  });

  it('saves which gem steps the route shows', async () => {
    render(<SettingsApp />);
    const rewards = screen.getByRole('button', { name: 'Rewards' });
    expect(screen.getByRole('button', { name: 'Rewards and buys' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await userEvent.click(rewards);
    expect(rewards).toHaveAttribute('aria-pressed', 'true');
    expect(commands.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({ gemHints: 'rewards' }),
    );
  });

  it('closes from its own title bar', async () => {
    render(<SettingsApp />);
    await userEvent.click(screen.getByRole('button', { name: 'Close settings' }));
    expect(commands.closeSettings).toHaveBeenCalled();
  });

  it('moves by the title bar, not by the close button', () => {
    render(<SettingsApp />);
    const bar = screen.getByText('Settings').closest('header');
    expect(bar).toHaveAttribute('data-tauri-drag-region');
    expect(screen.getByRole('button', { name: 'Close settings' })).not.toHaveAttribute(
      'data-tauri-drag-region',
    );
  });
});
