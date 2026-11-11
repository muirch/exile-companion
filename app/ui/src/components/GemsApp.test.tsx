import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import { linksMock } from '../ipc/mock';
import type { LinkGem } from '../ipc/types';
import { GemsApp, bySlot, connectors, gemLevel, parseNotes } from './GemsApp';

vi.mock('../ipc/commands', () => ({
  getLinks: vi.fn((set: number | null) => Promise.resolve(links(set))),
  getNotes: vi.fn(() => Promise.resolve(notes)),
  closeGems: vi.fn(() => Promise.resolve()),
}));

const NOTES = '^xFFFF77- Act 2 -^7\n\nFor our Bandit we ^x70FF70Help Alira^7.';
let notes: string | null = NOTES;

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
  notes = NOTES;
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

  it('colours the notes the way PoB does and keeps the line breaks', () => {
    expect(parseNotes('^xFFFF77Act 1^7 -\nMule ^2first\n\n\n\nThen go.')).toEqual([
      [
        [
          { text: 'Act 1', color: '#ffff77' },
          { text: ' -', color: null },
        ],
        [
          { text: 'Mule ', color: null },
          { text: 'first', color: '#00ff00' },
        ],
      ],
      [[{ text: 'Then go.', color: '#00ff00' }]],
    ]);
  });

  it('leaves a code it does not know and a colour too dark to read', () => {
    expect(parseNotes('^xZZ costs ^x101010dark')).toEqual([
      [
        [
          { text: '^xZZ costs ', color: null },
          { text: 'dark', color: null },
        ],
      ],
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

  it("shows the author's notes on their own tab", async () => {
    render(<GemsApp />);
    await userEvent.click(await screen.findByRole('button', { name: 'Notes' }));
    expect(screen.getByText('- Act 2 -')).toHaveStyle({ color: '#ffff77' });
    expect(screen.getByText('Help Alira')).toHaveStyle({ color: '#70ff70' });
    expect(screen.queryByRole('combobox', { name: 'Stage' })).toBeNull();
  });

  it('says when the author wrote no notes', async () => {
    notes = null;
    render(<GemsApp />);
    await userEvent.click(await screen.findByRole('button', { name: 'Notes' }));
    expect(screen.getByText("The build's author wrote no notes.")).toBeInTheDocument();
  });

  it('says what to do without a build', async () => {
    links = () => null;
    render(<GemsApp />);
    expect(await screen.findByText(/Follow a guide or import your build/)).toBeInTheDocument();
  });
});
