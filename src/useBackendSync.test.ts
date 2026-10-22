import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import * as commands from './ipc/commands';
import { useBackendSync } from './useBackendSync';

vi.mock('./ipc/env', () => ({ isTauri: () => true }));

let subscribed: () => void = () => {};
const listening = new Promise<void>((resolve) => {
  subscribed = resolve;
});

vi.mock('./ipc/events', async (importOriginal) => {
  const real = await importOriginal<typeof import('./ipc/events')>();
  const listen = vi.fn(() => listening.then(() => () => {}));
  return Object.fromEntries(
    Object.keys(real).map((name) => [name, name.startsWith('on') ? listen : real[name as keyof typeof real]]),
  );
});

vi.mock('./ipc/commands', async (importOriginal) => {
  const real = await importOriginal<typeof commands>();
  return Object.fromEntries(
    Object.keys(real).map((name) => [name, vi.fn(() => new Promise(() => {}))]),
  );
});

describe('useBackendSync', () => {
  it('reads the backend state only after every listener is in', async () => {
    renderHook(() => useBackendSync());
    await Promise.resolve();
    expect(commands.getState).not.toHaveBeenCalled();
    expect(commands.getSource).not.toHaveBeenCalled();

    subscribed();

    await waitFor(() => expect(commands.getSource).toHaveBeenCalled());
    expect(commands.getState).toHaveBeenCalled();
    expect(commands.getOverlayState).toHaveBeenCalled();
  });
});
