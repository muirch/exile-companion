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

  it('opens the assistant with its key next to it', async () => {
    render(<MenuApp />);
    const row = screen.getByRole('menuitem', { name: /Ask the assistant/ });
    expect(row).toHaveTextContent('Alt+A');
    await userEvent.click(row);
    expect(commands.openChat).toHaveBeenCalled();
  });

  it('asks for a second click before quitting', async () => {
    render(<MenuApp />);
    await userEvent.click(screen.getByRole('menuitem', { name: 'Quit Exile Companion' }));
    expect(commands.quitApp).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('menuitem', { name: 'Click again to quit' }));
    expect(commands.quitApp).toHaveBeenCalledTimes(1);
  });

  it('turns the full layout on, which means compact off', async () => {
    render(<MenuApp />);
    await userEvent.click(screen.getByRole('menuitemcheckbox', { name: /Full layout/ }));
    expect(commands.updateSettings).toHaveBeenCalledWith({ compact: false });
  });

  it('turns the passive tree on and shows it off until then', async () => {
    render(<MenuApp />);
    const row = await screen.findByRole('menuitemcheckbox', { name: /Passive tree/ });
    expect(row).toHaveTextContent('Off');
    await userEvent.click(row);
    expect(commands.setTreeShown).toHaveBeenCalledWith(true);
    expect(row).toHaveTextContent('On');
  });

  it('walks the rows with the arrow keys', async () => {
    render(<MenuApp />);
    expect(screen.getByRole('menuitemcheckbox', { name: /Show route/ })).toHaveFocus();
    await userEvent.keyboard('{ArrowDown}');
    expect(screen.getByRole('menuitemcheckbox', { name: /Full layout/ })).toHaveFocus();
    await userEvent.keyboard('{ArrowUp}{ArrowUp}');
    expect(screen.getByRole('menuitem', { name: 'Quit Exile Companion' })).toHaveFocus();
  });

  it('refreshes the imported character, and has no row without one', async () => {
    const { unmount } = render(<MenuApp />);
    expect(screen.queryByRole('menuitem', { name: /Refresh character/ })).toBeNull();
    unmount();
    vi.mocked(commands.getProfile).mockResolvedValue({ guide: null, character: null, link: { account: 'A#1', character: 'RealOne', realm: 'pc', imported: '' } });
    render(<MenuApp />);
    await userEvent.click(await screen.findByRole('menuitem', { name: /Refresh character/ }));
    expect(screen.getByText('RealOne')).toBeInTheDocument();
    expect(commands.refreshCharacter).toHaveBeenCalled();
  });
});
