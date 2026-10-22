import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import { MenuApp } from './MenuApp';

vi.mock('../ipc/commands', async (importOriginal) => {
  const real = await importOriginal<typeof commands>();
  return Object.fromEntries(
    Object.keys(real).map((name) => [name, vi.fn(() => Promise.resolve(null))]),
  );
});

describe('MenuApp', () => {
  it('steps the route from the menu', async () => {
    render(<MenuApp />);
    await userEvent.click(screen.getByRole('menuitem', { name: /Previous step/ }));
    expect(commands.routePrev).toHaveBeenCalled();
    await userEvent.click(screen.getByRole('menuitem', { name: /Next step/ }));
    expect(commands.routeNext).toHaveBeenCalled();
  });

  it('asks for a second click before quitting', async () => {
    render(<MenuApp />);
    await userEvent.click(screen.getByRole('menuitem', { name: 'Quit poebo' }));
    expect(commands.quitApp).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('menuitem', { name: 'Click again to quit' }));
    expect(commands.quitApp).toHaveBeenCalledTimes(1);
  });

  it('turns the full layout on, which means compact off', async () => {
    render(<MenuApp />);
    await userEvent.click(screen.getByRole('menuitemcheckbox', { name: /Full layout/ }));
    expect(commands.updateSettings).toHaveBeenCalledWith({ compact: false });
  });

  it('walks the rows with the arrow keys', async () => {
    render(<MenuApp />);
    expect(screen.getByRole('menuitemcheckbox', { name: /Show route/ })).toHaveFocus();
    await userEvent.keyboard('{ArrowDown}');
    expect(screen.getByRole('menuitemcheckbox', { name: /Full layout/ })).toHaveFocus();
    await userEvent.keyboard('{ArrowUp}{ArrowUp}');
    expect(screen.getByRole('menuitem', { name: 'Quit poebo' })).toHaveFocus();
  });
});
