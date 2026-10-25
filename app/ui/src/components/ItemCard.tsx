import type { GearCard, GemCard, ItemCard as Card } from '../ipc/types';
import { TooltipHeader, TooltipSep } from './Tooltip';

const TINT = 'var(--color-currency)';

const VERDICT: Record<GearCard['verdict'], string> = {
  upgrade: 'Upgrade',
  downgrade: 'Worse than yours',
  mixed: 'Trade-off',
  same: 'About the same',
};

function signed(n: number, digits = 1): string {
  const r = Number(n.toFixed(digits));
  return `${r > 0 ? '+' : ''}${r}`;
}

/** "+4.2% DPS · -1.5% EHP · +12 life", only what moved. */
export function gearNumbers(c: GearCard): string {
  const parts: string[] = [];
  if (Math.abs(c.dpsPct) >= 0.1) parts.push(`${signed(c.dpsPct)}% DPS`);
  if (Math.abs(c.ehpPct) >= 0.1) parts.push(`${signed(c.ehpPct)}% EHP`);
  if (Math.abs(c.life) >= 1) parts.push(`${signed(c.life, 0)} life`);
  return parts.join(' · ');
}

export function resistLine(c: GearCard): string {
  return c.resists
    .filter((r) => Math.abs(r.change) >= 1)
    .map((r) => `${r.element} ${signed(r.change, 0)}% (${Math.round(r.after)}%)`)
    .join(' · ');
}

function Gear({ card, needsLevel }: { card: GearCard; needsLevel: number | null }): React.JSX.Element {
  const numbers = gearNumbers(card);
  const resists = resistLine(card);
  const worth = card.verdict === 'upgrade' || card.verdict === 'mixed';
  return (
    <>
      <p className="text-[15px] leading-snug text-value">{VERDICT[card.verdict]}</p>
      {needsLevel != null && (
        <p className="text-[13px] leading-snug text-plain">
          Needs level {needsLevel}.{worth && " You'll get a reminder then."}
        </p>
      )}
      {numbers && <p className="text-[14px] leading-snug text-value">{numbers}</p>}
      {resists && <p className="text-[13px] leading-snug text-plain">{resists}</p>}
      {card.partly && (
        <p className="font-body text-[13px] leading-snug text-plain italic">
          Path of Building can't count all of it.
        </p>
      )}
    </>
  );
}

function Gem({ card }: { card: GemCard }): React.JSX.Element {
  return (
    <>
      {card.uses.length === 0 ? (
        <p className="text-[15px] leading-snug text-value">Your build doesn't use it</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {card.uses.map((u) => (
            <li key={`${u.set}|${u.skill}|${u.links.join()}`} className="leading-snug">
              {u.set && <span className="block text-[13px] text-plain">{u.set}</span>}
              <span className="text-[15px] text-value">{u.skill}</span>
              {u.links.length > 0 && (
                <span className="block text-[14px] text-gem">{u.links.join(' · ')}</span>
              )}
            </li>
          ))}
        </ul>
      )}
      {card.get && (
        <>
          <TooltipSep tint={TINT} />
          <p className="text-[14px] leading-snug text-plain">{card.get}</p>
        </>
      )}
    </>
  );
}

function Ready({ card }: { card: Extract<Card, { kind: 'ready' }> }): React.JSX.Element {
  return (
    <>
      {card.gems.length > 0 && (
        <p className="text-[15px] leading-snug text-gem">{card.gems.join(' · ')}</p>
      )}
      {card.items.map((i) => (
        <p key={i.slot} className="leading-snug">
          <span className="text-[15px] text-value">{i.name}</span>
          <span className="block text-[13px] text-plain">{i.slot}</span>
        </p>
      ))}
    </>
  );
}

function Kitava({ card }: { card: Extract<Card, { kind: 'kitava' }> }): React.JSX.Element {
  return (
    <>
      <p className="text-[15px] leading-snug text-value">
        {card.short.map((r) => `${r.element} ${Math.round(r.after)}%`).join(' · ')}
      </p>
      <p className="font-body text-[13px] leading-snug text-plain italic">
        Below the 75% cap once he's dead. Fix it before the fight, the penalty stays.
      </p>
    </>
  );
}

function title(card: Card): string {
  if (card.kind === 'ready') return `Level ${card.level}`;
  if (card.kind === 'kitava') return 'Kitava ahead';
  return card.name;
}

function subtitle(card: Card): string | null {
  if (card.kind === 'gem') return 'Gem';
  if (card.kind === 'ready') return 'Usable now';
  if (card.kind === 'kitava') return `Resistances ${card.penalty}% after him`;
  if (card.kind === 'gear') {
    return card.card.replaces ? `${card.card.slot} · instead of ${card.card.replaces}` : card.card.slot;
  }
  return null;
}

function Body({ card }: { card: Card }): React.JSX.Element {
  switch (card.kind) {
    case 'measuring':
      return (
        <p role="status" className="flex items-center justify-center gap-2 text-[13px] text-plain">
          <span className="chat-pulse" aria-hidden="true" />
          Measuring with Path of Building…
        </p>
      );
    case 'missing':
      return <p className="font-body text-[13px] leading-snug text-plain italic">{card.message}</p>;
    case 'gem':
      return <Gem card={card} />;
    case 'gear':
      return <Gear card={card.card} needsLevel={card.needsLevel} />;
    case 'ready':
      return <Ready card={card} />;
    case 'kitava':
      return <Kitava card={card} />;
  }
}

/** What a copied item means for the build, as one more tooltip over the route. */
export function ItemCard({ card, compact }: { card: Card; compact: boolean }): React.JSX.Element {
  const sub = subtitle(card);
  if (compact) {
    return (
      <section aria-label="Item check" className="tooltip flex flex-col gap-1 px-3 pt-1.5 pb-2">
        <p className="truncate text-[14px] text-value">{title(card)}</p>
        {sub && <p className="truncate text-[13px] text-plain">{sub}</p>}
        <Body card={card} />
      </section>
    );
  }
  return (
    <section aria-label="Item check" className="tooltip">
      <TooltipHeader tint={TINT} title={title(card)} subtitle={sub} />
      <div className="flex flex-col gap-1 px-4 pt-2 pb-3">
        <Body card={card} />
      </div>
    </section>
  );
}
