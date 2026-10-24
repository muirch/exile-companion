import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import type { ChatView } from '../ipc/types';
import { ChatSettings } from './SettingsApp';

let view: ChatView = { hasKey: false, available: true, busy: false };

vi.mock('../ipc/commands', async (importOriginal) => {
  const real = await importOriginal<typeof commands>();
  const stubs = Object.fromEntries(
    Object.keys(real).map((name) => [name, vi.fn(() => Promise.resolve(null))]),
  );
  return {
    ...stubs,
    getChat: vi.fn(() => Promise.resolve(view)),
    getSettings: vi.fn(() =>
      Promise.resolve({ chat: { model: 'claude-sonnet-5-5', effort: 'medium' } }),
    ),
    setApiKey: vi.fn(() => Promise.resolve({ ...view, hasKey: true })),
    forgetApiKey: vi.fn(() => Promise.resolve({ ...view, hasKey: false })),
  };
});

beforeEach(() => {
  view = { hasKey: false, available: true, busy: false };
  vi.clearAllMocks();
});

describe('ChatSettings', () => {
  it('saves a pasted key and then only says it is saved', async () => {
    render(<ChatSettings />);
    const field = await screen.findByLabelText('Anthropic API key');
    expect(field).toHaveAttribute('type', 'password');
    await userEvent.type(field, '  sk-ant-test  {Enter}');
    expect(commands.setApiKey).toHaveBeenCalledWith('  sk-ant-test  ');
    expect(await screen.findByText('Saved')).toBeInTheDocument();
    expect(screen.queryByLabelText('Anthropic API key')).toBeNull();
  });

  it('forgets the key on a second click', async () => {
    view = { hasKey: true, available: true, busy: false };
    render(<ChatSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Forget' }));
    expect(commands.forgetApiKey).not.toHaveBeenCalled();
    await new Promise((r) => setTimeout(r, 550));
    await userEvent.click(screen.getByRole('button', { name: 'Click again to forget' }));
    expect(commands.forgetApiKey).toHaveBeenCalled();
    expect(await screen.findByLabelText('Anthropic API key')).toBeInTheDocument();
  });

  it('switches model and effort', async () => {
    view = { hasKey: true, available: true, busy: false };
    render(<ChatSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Opus 5.5' }));
    expect(commands.updateSettings).toHaveBeenCalledWith({
      chat: { model: 'claude-opus-5-5', effort: 'medium' },
    });
    await userEvent.click(screen.getByRole('button', { name: 'Low' }));
    expect(commands.updateSettings).toHaveBeenLastCalledWith({
      chat: { model: 'claude-opus-5-5', effort: 'low' },
    });
  });

  it('stays out of a build without the assistant', async () => {
    view = { hasKey: false, available: false, busy: false };
    const { container } = render(<ChatSettings />);
    await screen.findByText(() => true).catch(() => null);
    expect(container).toBeEmptyDOMElement();
  });
});
