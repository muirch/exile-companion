import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { GearCard } from '../ipc/types';
import { ItemCard, gearNumbers, resistLine } from './ItemCard';

const GEAR: GearCard = {
  slot: 'Ring 1',
  replaces: 'Iron Ring',
  verdict: 'upgrade',
  dpsPct: 4.23,
  ehpPct: -1.5,
  life: 12,
  resists: [
    { element: 'Fire', change: 15, after: 55 },
    { element: 'Cold', change: 0.2, after: 30 },
  ],
  partly: false,
};

describe('gear numbers', () => {
  it('lists only what moved, with signs', () => {
    expect(gearNumbers(GEAR)).toBe('+4.2% DPS · -1.5% EHP · +12 life');
    expect(gearNumbers({ ...GEAR, dpsPct: 0, ehpPct: 0.05, life: 0 })).toBe('');
  });

  it('shows resistance changes with the total after', () => {
    expect(resistLine(GEAR)).toBe('Fire +15% (55%)');
  });
});

describe('ItemCard', () => {
  it('shows the verdict, the slot and what it replaces', () => {
    render(<ItemCard compact={false} card={{ kind: 'gear', name: 'Death Song, Iron Ring', card: GEAR, needsLevel: null }} />);
    expect(screen.getByRole('heading', { name: 'Death Song, Iron Ring' })).toBeInTheDocument();
    expect(screen.getByText('Ring 1 · instead of Iron Ring')).toBeInTheDocument();
    expect(screen.getByText('Upgrade')).toBeInTheDocument();
    expect(screen.queryByText("Path of Building can't count all of it.")).not.toBeInTheDocument();
  });

  it('says when PoB could not count all of the item', () => {
    render(<ItemCard compact card={{ kind: 'gear', name: 'Ring', card: { ...GEAR, partly: true }, needsLevel: null }} />);
    expect(screen.getByText("Path of Building can't count all of it.")).toBeInTheDocument();
  });

  it('says an upgrade waits for a level and will be reminded', () => {
    render(<ItemCard compact card={{ kind: 'gear', name: 'Ring', card: GEAR, needsLevel: 24 }} />);
    expect(screen.getByText("Needs level 24. You'll get a reminder then.")).toBeInTheDocument();
  });

  it('only names the level for an item that is no better', () => {
    render(
      <ItemCard
        compact
        card={{ kind: 'gear', name: 'Ring', card: { ...GEAR, verdict: 'downgrade' }, needsLevel: 24 }}
      />,
    );
    expect(screen.getByText('Needs level 24.')).toBeInTheDocument();
  });

  it('lists what a level-up made usable', () => {
    render(
      <ItemCard
        compact={false}
        card={{
          kind: 'ready',
          level: 16,
          gems: ['Herald of Ash', 'Onslaught'],
          items: [{ name: 'Death Song, Short Bow', slot: 'Weapon 1', level: 16 }],
        }}
      />,
    );
    expect(screen.getByRole('heading', { name: 'Level 16' })).toBeInTheDocument();
    expect(screen.getByText('Usable now')).toBeInTheDocument();
    expect(screen.getByText('Herald of Ash · Onslaught')).toBeInTheDocument();
    expect(screen.getByText('Weapon 1')).toBeInTheDocument();
  });

  it('warns about resistances below the cap after Kitava', () => {
    render(
      <ItemCard
        compact={false}
        card={{
          kind: 'kitava',
          penalty: -30,
          short: [
            { element: 'Fire', after: 60.4 },
            { element: 'Lightning', after: 41 },
          ],
        }}
      />,
    );
    expect(screen.getByRole('heading', { name: 'Kitava ahead' })).toBeInTheDocument();
    expect(screen.getByText('Resistances -30% after him')).toBeInTheDocument();
    expect(screen.getByText('Fire 60% · Lightning 41%')).toBeInTheDocument();
  });

  it('lists where the build uses a gem and where to get it', () => {
    render(
      <ItemCard
        compact={false}
        card={{
          kind: 'gem',
          name: 'Added Fire Damage Support',
          uses: [{ set: 'Act 4', skill: 'Fireball', links: ['Spell Echo', 'Arcane Surge'] }],
          get: 'Sold by Nessa in act 1',
        }}
      />,
    );
    expect(screen.getByText('Act 4')).toBeInTheDocument();
    expect(screen.getByText('Spell Echo · Arcane Surge')).toBeInTheDocument();
    expect(screen.getByText('Sold by Nessa in act 1')).toBeInTheDocument();
  });

  it('says when the build does not use the gem', () => {
    render(<ItemCard compact card={{ kind: 'gem', name: 'Cleave', uses: [], get: null }} />);
    expect(screen.getByText("Your build doesn't use it")).toBeInTheDocument();
  });

  it('shows measuring while PoB works', () => {
    render(<ItemCard compact card={{ kind: 'measuring', name: 'Ring' }} />);
    expect(screen.getByRole('status')).toHaveTextContent('Measuring with Path of Building…');
  });

  it('shows why there is nothing to measure', () => {
    render(
      <ItemCard compact card={{ kind: 'missing', name: 'Ring', message: 'Import your build first.' }} />,
    );
    expect(screen.getByText('Import your build first.')).toBeInTheDocument();
  });
});
