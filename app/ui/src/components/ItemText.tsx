import type { ItemView } from '../ipc/types';

export const RARITY: Record<ItemView['rarity'], string> = {
  normal: 'text-normal',
  magic: 'text-magic',
  rare: 'text-rare',
  unique: 'text-unique',
  relic: 'text-unique',
};

/** The same colours for drawing, a jewel's area on the tree. */
export const RARITY_COLOR: Record<ItemView['rarity'], string> = {
  normal: 'var(--color-normal)',
  magic: 'var(--color-magic)',
  rare: 'var(--color-rare)',
  unique: 'var(--color-unique)',
  relic: 'var(--color-unique)',
};

const TONE: Record<ItemView['mods'][number]['kind'], string> = {
  enchant: 'text-crafted',
  implicit: 'text-magic',
  explicit: 'text-magic',
  crafted: 'text-crafted',
  fractured: 'text-fractured',
};

/** "Judgement Staff" for a rare the author left as PoB's "New Item", the name and base otherwise. */
export function itemTitle(item: ItemView): { name: string; base: string | null } {
  if (item.name === 'New Item' && item.base) return { name: item.base, base: null };
  return { name: item.name, base: item.base };
}

/** An item the way the game's tooltip reads: name in its rarity colour, base, then the mods. */
export function ItemText({ item }: { item: ItemView }): React.JSX.Element {
  const { name, base } = itemTitle(item);
  const implicits = item.mods.filter((m) => m.kind === 'implicit' || m.kind === 'enchant');
  const rest = item.mods.filter((m) => m.kind !== 'implicit' && m.kind !== 'enchant');
  const facts = [
    item.radius && `Radius ${item.radius}`,
    item.quality && `Quality ${item.quality}%`,
    item.sockets && `Sockets ${item.sockets}`,
    item.levelReq && `Level ${item.levelReq}`,
  ].filter(Boolean);
  return (
    <div className="flex flex-col gap-0.5" data-testid="item-text">
      <p className={`font-caps text-[15px] leading-snug ${RARITY[item.rarity]}`}>{name}</p>
      {base && <p className={`font-caps text-[13px] leading-snug ${RARITY[item.rarity]}`}>{base}</p>}
      {facts.length > 0 && <p className="font-body text-[12px] text-label">{facts.join(' · ')}</p>}
      {[implicits, rest].map(
        (mods, i) =>
          mods.length > 0 && (
            <ul key={i} className="mt-1 flex flex-col font-body text-[13px] leading-snug">
              {mods.map((m, j) => (
                <li key={`${j}:${m.text}`} className={TONE[m.kind]}>
                  {m.text}
                </li>
              ))}
            </ul>
          ),
      )}
      {item.corrupted && <p className="font-body text-[13px] text-corrupted">Corrupted</p>}
    </div>
  );
}
