import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import { linksMock } from '../ipc/mock';
import type { LinkGem } from '../ipc/types';
import { GemsApp, bySlot, connectors, gemLevel } from './GemsApp';

vi.mock('../ipc/commands', () => ({
  getLinks: vi.fn((set: number | null) => Promise.resolve(links(set))),
  closeGems: vi.fn(() => Promise.resolve()),
}));

let links: (set: number | null) => ReturnType<typeof linksMock> | null = linksMock;

const gem = (name: string, support: boolean): LinkGem => ({
  name,
  color: 'red',
  support,
  level: 20,
  quality: 20,
  requiredLevel: 12,
  get: null,
});

beforeEach(() => {
  links = linksMock;
  vi.clearAllMocks();
});

describe('gem links', () => {
  it('hangs each support from the skill before it', () => {
    expect(
      connectors([gem('Raise Spectre', false), gem('Meat Shield', true), gem('Animate Guardian', false), gem('Minion Life', true)]),
    ).toEqual([null, '└', null, '└']);
    expect(connectors([gem('Sunder', false), gem('Ruthless', true), gem('Bleed', true)])).toEqual([null, '├', '└']);
  });

  it('puts the groups of one item under one slot', () => {
    const slots = bySlot(linksMock(1).groups);
    expect(slots.map((s) => [s.slot, s.groups.length])).toEqual([
      ['Body Armour', 2],
      ['Boots', 1],
      ['Helmet', 1],
      ['Gloves', 1],
    ]);
  });

  it('says level and quality as PoB sets them', () => {
    expect(gemLevel(gem('Sunder', false))).toBe('Level 20, quality 20%');
    expect(gemLevel({ ...gem('Sunder', false), quality: 0 })).toBe('Level 20');
  });
});

describe('GemsApp', () => {
  it('shows the stage the character is at, by slot', async () => {
    render(<GemsApp />);
    const body = await screen.findByRole('region', { name: 'Body Armour' });
    expect(within(body).getByText('Sunder')).toBeInTheDocument();
    expect(within(body).getByText('Animate Guardian')).toBeInTheDocument();
    expect(commands.getLinks).toHaveBeenCalledWith(null);
  });

  it('shows where a gem comes from on hover', async () => {
    render(<GemsApp />);
    await userEvent.hover(await screen.findByRole('button', { name: 'Momentum Support' }));
    const card = screen.getByRole('tooltip');
    expect(card).toHaveTextContent('Support gem');
    expect(card).toHaveTextContent('From the Ranger mule');
    await userEvent.unhover(screen.getByRole('button', { name: 'Momentum Support' }));
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('switches the stage and closes with Esc', async () => {
    render(<GemsApp />);
    await userEvent.click(await screen.findByRole('combobox', { name: 'Stage' }));
    await userEvent.click(screen.getByRole('option', { name: 'Level 1-12' }));
    expect(commands.getLinks).toHaveBeenLastCalledWith(0);
    await userEvent.keyboard('{Escape}');
    expect(commands.closeGems).toHaveBeenCalled();
  });

  it('says what to do without a build', async () => {
    links = () => null;
    render(<GemsApp />);
    expect(await screen.findByText(/Follow a guide or import your build/)).toBeInTheDocument();
  });
});
