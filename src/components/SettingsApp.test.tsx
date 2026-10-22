import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import { useAppStore } from '../store/useAppStore';
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
  it('lets the player set the act of each preset of the build', async () => {
    act(() =>
      useAppStore.setState({
        build: {
          class_name: 'Ranger',
          ascend_class_name: null,
          level: 90,
          active_spec_title: null,
          main_skill: null,
          spec_count: 1,
          gem_count: 3,
          presets: [
            { title: 'Leveling', gem_count: 2, auto_level: null },
            { title: '-- separator --', gem_count: 0, auto_level: null },
            { title: 'Switch', gem_count: 1, auto_level: null },
          ],
        },
      }),
    );
    render(<SettingsApp />);
    expect(screen.getByText('No gems')).toBeInTheDocument();
    await userEvent.selectOptions(screen.getByLabelText('Stage of Switch'), 'From act 4');
    expect(commands.updateSettings).toHaveBeenCalledWith({
      presetStages: ['auto', 'auto', { act: 4 }],
    });
    act(() => useAppStore.setState({ build: null }));
  });

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
    expect(commands.updateSettings).toHaveBeenCalledWith({ gemHints: 'rewards' });
  });

  it('switches the overlay to compact and sets the fade time', async () => {
    render(<SettingsApp />);
    await userEvent.click(screen.getByRole('button', { name: 'Compact' }));
    expect(commands.updateSettings).toHaveBeenCalledWith({ compact: true });
    await userEvent.click(screen.getByRole('button', { name: '10 s' }));
    expect(commands.updateSettings).toHaveBeenCalledWith({ fadeAfter: 10 });
  });

  it('asks for a second click before starting the route over', async () => {
    render(<SettingsApp />);
    await userEvent.click(screen.getByRole('button', { name: 'Start over' }));
    expect(commands.routeReset).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Click again to start over' }));
    expect(commands.routeReset).toHaveBeenCalledTimes(1);
  });

  it('keeps log replay behind Testing', async () => {
    render(<SettingsApp />);
    expect(screen.queryByRole('button', { name: 'Replay a log' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Show' }));
    expect(screen.getByRole('button', { name: 'Replay a log' })).toBeInTheDocument();
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
