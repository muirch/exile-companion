import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import type { HotkeySettings } from '../ipc/types';
import { HotkeySection, holdLabel, shortcutLabel } from './HotkeySection';

vi.mock('../ipc/commands', () => ({
  getHotkeys: vi.fn(),
  setHotkeys: vi.fn(),
  pauseHotkeys: vi.fn(),
  resumeHotkeys: vi.fn(),
}));

const CFG: HotkeySettings = {
  hotkeys: {
    menuHold: 'Backquote',
    moveOverlay: 'Alt+KeyM',
    routePrev: 'Alt+ArrowLeft',
    routeNext: 'Alt+ArrowRight',
    relog: 'F9',
  },
  relogEnabled: false,
};

const nextStepButton = (): Promise<HTMLElement> =>
  screen.findByRole('button', { name: 'Alt+Right' });

describe('shortcutLabel', () => {
  it.each([
    ['Alt+KeyM', 'Alt+M'],
    ['Alt+ArrowLeft', 'Alt+Left'],
    ['Ctrl+Shift+Digit3', 'Ctrl+Shift+3'],
    ['Numpad5', 'Num 5'],
    ['Ctrl+Backquote', 'Ctrl+`'],
    ['F9', 'F9'],
    ['', 'Not set'],
  ])('%s reads %s', (shortcut, label) => {
    expect(shortcutLabel(shortcut)).toBe(label);
  });
});

describe('holdLabel', () => {
  it('names the hold keys and passes others through', () => {
    expect(holdLabel('Backquote')).toBe('Tilde');
    expect(holdLabel('Tab')).toBe('Tab');
    expect(holdLabel('KeyQ')).toBe('KeyQ');
  });
});

describe('HotkeySection', () => {
  beforeEach(() => {
    vi.mocked(commands.getHotkeys).mockResolvedValue(structuredClone(CFG));
    vi.mocked(commands.setHotkeys).mockResolvedValue([]);
    vi.mocked(commands.pauseHotkeys).mockResolvedValue();
    vi.mocked(commands.resumeHotkeys).mockResolvedValue([]);
  });

  it('records a new shortcut with modifiers', async () => {
    const user = userEvent.setup();
    render(<HotkeySection />);
    await user.click(await nextStepButton());

    expect(screen.getByRole('button', { name: 'Press keys…' })).toHaveAttribute('aria-pressed', 'true');
    expect(commands.pauseHotkeys).toHaveBeenCalled();

    await user.keyboard('{Control>}{Shift>}n{/Shift}{/Control}');

    expect(commands.setHotkeys).toHaveBeenCalledWith({
      ...CFG,
      hotkeys: { ...CFG.hotkeys, routeNext: 'Ctrl+Shift+KeyN' },
    });
    expect(screen.getByRole('button', { name: 'Ctrl+Shift+N' })).toBeInTheDocument();
  });

  it('waits for a non-modifier key', async () => {
    const user = userEvent.setup();
    render(<HotkeySection />);
    await user.click(await nextStepButton());
    await user.keyboard('{Alt}');

    expect(commands.setHotkeys).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Press keys…' })).toBeInTheDocument();
  });

  it('cancels on Esc and resumes the old bindings', async () => {
    const user = userEvent.setup();
    render(<HotkeySection />);
    await user.click(await nextStepButton());
    await user.keyboard('{Escape}');

    expect(commands.setHotkeys).not.toHaveBeenCalled();
    expect(commands.resumeHotkeys).toHaveBeenCalled();
    expect(await nextStepButton()).toBeInTheDocument();
  });

  it('clears a binding on Backspace', async () => {
    const user = userEvent.setup();
    render(<HotkeySection />);
    await user.click(await nextStepButton());
    await user.keyboard('{Backspace}');

    expect(commands.setHotkeys).toHaveBeenCalledWith({
      ...CFG,
      hotkeys: { ...CFG.hotkeys, routeNext: '' },
    });
    expect(screen.getByRole('button', { name: 'Not set' })).toBeInTheDocument();
  });

  it('marks a binding the backend rejected', async () => {
    vi.mocked(commands.setHotkeys).mockResolvedValue([
      { action: 'routeNext', shortcut: 'Alt+KeyF', error: 'taken' },
    ]);
    const user = userEvent.setup();
    render(<HotkeySection />);
    await user.click(await nextStepButton());
    await user.keyboard('{Alt>}f{/Alt}');

    expect(await screen.findByText('Taken or invalid')).toBeInTheDocument();
  });

  it('toggles relog', async () => {
    const user = userEvent.setup();
    render(<HotkeySection />);
    const toggle = await screen.findByRole('switch');
    expect(toggle).toHaveAttribute('aria-checked', 'false');

    await user.click(toggle);

    expect(toggle).toHaveAttribute('aria-checked', 'true');
    expect(commands.setHotkeys).toHaveBeenCalledWith({ ...CFG, relogEnabled: true });
  });
});
