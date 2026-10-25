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
    render(<ItemCard compact={false} card={{ kind: 'gear', name: 'Death Song, Iron Ring', card: GEAR }} />);
    expect(screen.getByRole('heading', { name: 'Death Song, Iron Ring' })).toBeInTheDocument();
    expect(screen.getByText('Ring 1 · instead of Iron Ring')).toBeInTheDocument();
    expect(screen.getByText('Upgrade')).toBeInTheDocument();
    expect(screen.queryByText("Path of Building can't count all of it.")).not.toBeInTheDocument();
  });

  it('says when PoB could not count all of the item', () => {
    render(<ItemCard compact card={{ kind: 'gear', name: 'Ring', card: { ...GEAR, partly: true } }} />);
    expect(screen.getByText("Path of Building can't count all of it.")).toBeInTheDocument();
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
