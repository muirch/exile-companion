import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import type { ChatConfig, ChatView } from '../ipc/types';
import { ChatSettings } from './SettingsApp';

const NO_KEYS: ChatView = {
  keys: { anthropic: false, openRouter: false, custom: false },
  available: true,
  busy: false,
  missing: 'Add your Anthropic API key in the settings.',
  model: 'Sonnet 5.5',
};
const CONFIG: ChatConfig = {
  provider: 'anthropic',
  model: 'claude-sonnet-5-5',
  effort: 'medium',
  routerModel: '',
  customUrl: '',
  customModel: '',
};
let view: ChatView = NO_KEYS;
let config: ChatConfig = CONFIG;

vi.mock('../ipc/commands', async (importOriginal) => {
  const real = await importOriginal<typeof commands>();
  const stubs = Object.fromEntries(
    Object.keys(real).map((name) => [name, vi.fn(() => Promise.resolve(null))]),
  );
  return {
    ...stubs,
    getChat: vi.fn(() => Promise.resolve(view)),
    getSettings: vi.fn(() => Promise.resolve({ chat: config })),
    setApiKey: vi.fn((p: 'anthropic' | 'openRouter' | 'custom') =>
      Promise.resolve({ ...view, keys: { ...view.keys, [p]: true } }),
    ),
    forgetApiKey: vi.fn((p: 'anthropic' | 'openRouter' | 'custom') =>
      Promise.resolve({ ...view, keys: { ...view.keys, [p]: false } }),
    ),
  };
});

beforeEach(() => {
  view = NO_KEYS;
  config = CONFIG;
  vi.clearAllMocks();
});

describe('ChatSettings', () => {
  it('saves a pasted key for the picked provider and then only says it is saved', async () => {
    render(<ChatSettings />);
    const field = await screen.findByLabelText('API key');
    expect(field).toHaveAttribute('type', 'password');
    await userEvent.type(field, 'sk-ant-test{Enter}');
    expect(commands.setApiKey).toHaveBeenCalledWith('anthropic', 'sk-ant-test');
    expect(await screen.findByText('Saved')).toBeInTheDocument();
    expect(screen.queryByLabelText('API key')).toBeNull();
  });

  it('forgets the key on a second click', async () => {
    view = { ...NO_KEYS, keys: { ...NO_KEYS.keys, anthropic: true } };
    render(<ChatSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Forget' }));
    expect(commands.forgetApiKey).not.toHaveBeenCalled();
    await new Promise((r) => setTimeout(r, 550));
    await userEvent.click(screen.getByRole('button', { name: 'Click again to forget' }));
    expect(commands.forgetApiKey).toHaveBeenCalledWith('anthropic');
    expect(await screen.findByLabelText('API key')).toBeInTheDocument();
  });

  it('switches model and effort', async () => {
    render(<ChatSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Opus 5.5' }));
    expect(commands.updateSettings).toHaveBeenCalledWith({
      chat: { ...CONFIG, model: 'claude-opus-5-5' },
    });
    await userEvent.click(screen.getByRole('button', { name: 'Low' }));
    expect(commands.updateSettings).toHaveBeenLastCalledWith({
      chat: { ...CONFIG, model: 'claude-opus-5-5', effort: 'low' },
    });
  });

  it('takes an OpenRouter model by its id when the field is left', async () => {
    render(<ChatSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'OpenRouter' }));
    expect(commands.updateSettings).toHaveBeenCalledWith({ chat: { ...CONFIG, provider: 'openRouter' } });
    const model = screen.getByLabelText('OpenRouter model');
    await userEvent.type(model, ' vendor/model ');
    expect(commands.updateSettings).toHaveBeenCalledTimes(1);
    await userEvent.tab();
    expect(commands.updateSettings).toHaveBeenLastCalledWith({
      chat: { ...CONFIG, provider: 'openRouter', routerModel: 'vendor/model' },
    });
    await userEvent.type(screen.getByLabelText('API key'), 'sk-or-1{Enter}');
    expect(commands.setApiKey).toHaveBeenCalledWith('openRouter', 'sk-or-1');
  });

  it('asks a custom server for its address and model', async () => {
    config = { ...CONFIG, provider: 'custom' };
    render(<ChatSettings />);
    await userEvent.type(await screen.findByLabelText('Server address'), 'http://localhost:1234/v1{Enter}');
    expect(commands.updateSettings).toHaveBeenLastCalledWith({
      chat: { ...config, customUrl: 'http://localhost:1234/v1' },
    });
    expect(screen.getByLabelText('Model on the server')).toBeInTheDocument();
  });

  it('stays out of a build without the assistant', async () => {
    view = { ...NO_KEYS, available: false };
    const { container } = render(<ChatSettings />);
    await new Promise((r) => setTimeout(r, 0));
    expect(container).toBeEmptyDOMElement();
  });
});
