import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import type { ChatEvent, ChatView, TurnOver } from '../ipc/types';
import { ChatApp } from './ChatApp';

let view: ChatView = { hasKey: true, available: true, busy: false };
let emit: (e: ChatEvent) => void = () => {};
let over: (o: TurnOver) => void = () => {};

vi.mock('../ipc/commands', () => ({
  getChat: vi.fn(() => Promise.resolve(view)),
  chatSend: vi.fn(() => Promise.resolve()),
  chatStop: vi.fn(() => Promise.resolve()),
  chatNew: vi.fn(() => Promise.resolve()),
  closeChat: vi.fn(() => Promise.resolve()),
  openSettings: vi.fn(() => Promise.resolve()),
}));

vi.mock('../ipc/events', () => ({
  onChatEvent: (cb: (e: ChatEvent) => void) => {
    emit = cb;
    return Promise.resolve(() => {});
  },
  onChatOver: (cb: (o: TurnOver) => void) => {
    over = cb;
    return Promise.resolve(() => {});
  },
  onChatOpened: () => Promise.resolve(() => {}),
}));

const DONE: TurnOver = { stop: 'done', message: null, usage: null, dollars: 0.021 };

beforeEach(() => {
  view = { hasKey: true, available: true, busy: false };
  vi.clearAllMocks();
});

async function askQuestion(text: string): Promise<void> {
  const box = await screen.findByRole('textbox', { name: 'Question for the assistant' });
  await userEvent.type(box, `${text}{Enter}`);
}

describe('ChatApp', () => {
  it('sends the question with Enter and streams the answer', async () => {
    render(<ChatApp />);
    await askQuestion('What next?');
    expect(commands.chatSend).toHaveBeenCalledWith('What next?');
    act(() => emit({ kind: 'tool', name: 'passive_value' }));
    expect(screen.getByRole('status')).toHaveTextContent('Measuring passives');
    act(() => emit({ kind: 'text', text: 'Take Herbalism.\n\nIt adds 12% life.' }));
    expect(screen.queryByRole('status')).toBeNull();
    expect(screen.getByText('Take Herbalism.')).toBeInTheDocument();
    expect(screen.queryByText('It adds 12% life.')).toBeNull();
    expect(screen.getByRole('button', { name: 'Stop' })).toBeInTheDocument();
    act(() => over(DONE));
    expect(screen.getByText('2¢')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Details' }));
    expect(screen.getByText('It adds 12% life.')).toBeInTheDocument();
  });

  it('keeps Shift+Enter as a new line', async () => {
    render(<ChatApp />);
    const box = await screen.findByRole('textbox', { name: 'Question for the assistant' });
    await userEvent.type(box, 'Line one{Shift>}{Enter}{/Shift}line two');
    expect(commands.chatSend).not.toHaveBeenCalled();
    expect(box).toHaveValue('Line one\nline two');
  });

  it('stops a running answer', async () => {
    render(<ChatApp />);
    await askQuestion('q');
    await userEvent.click(screen.getByRole('button', { name: 'Stop' }));
    expect(commands.chatStop).toHaveBeenCalled();
    act(() => over({ stop: 'stopped', message: null, usage: null, dollars: null }));
    expect(screen.getByText('Stopped.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Ask' })).toBeInTheDocument();
  });

  it('shows what went wrong and how to fix it', async () => {
    render(<ChatApp />);
    await askQuestion('q');
    act(() =>
      over({
        stop: 'error',
        message: 'Anthropic refused the API key. Check it in the settings.',
        usage: null,
        dollars: null,
      }),
    );
    expect(screen.getByRole('alert')).toHaveTextContent('refused the API key');
  });

  it('asks for a key before anything else', async () => {
    view = { hasKey: false, available: true, busy: false };
    render(<ChatApp />);
    await userEvent.click(await screen.findByRole('button', { name: 'Open settings' }));
    expect(commands.openSettings).toHaveBeenCalled();
    expect(screen.getByRole('textbox', { name: 'Question for the assistant' })).toBeDisabled();
  });

  it('asks a suggested question in one click', async () => {
    render(<ChatApp />);
    await userEvent.click(await screen.findByRole('button', { name: 'What should I do next?' }));
    expect(commands.chatSend).toHaveBeenCalledWith('What should I do next?');
  });

  it('starts over with New chat and closes with Esc', async () => {
    render(<ChatApp />);
    await askQuestion('q');
    act(() => over(DONE));
    await userEvent.click(screen.getByRole('button', { name: 'New chat' }));
    expect(commands.chatNew).toHaveBeenCalled();
    expect(await screen.findByText('Ask about your build.')).toBeInTheDocument();
    await userEvent.keyboard('{Escape}');
    expect(commands.closeChat).toHaveBeenCalled();
  });
});
