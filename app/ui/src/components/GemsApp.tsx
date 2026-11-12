import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';

import { closeGems, getLinks, getNotes } from '../ipc/commands';
import { isTauri } from '../ipc/env';
import { onGemsOpened } from '../ipc/events';
import type { GemColor, LinkGem, LinkGroup, Links } from '../ipc/types';
import { Button, Choice, Select } from '../kit';
import { ItemText, itemTitle, RARITY } from './ItemText';
import { Brand } from './Tooltip';

const COLOR: Record<GemColor, string> = {
  red: 'text-gem-red',
  green: 'text-gem-green',
  blue: 'text-gem-blue',
  white: 'text-normal',
};

const CARD_ID = 'gem-card';
const CARD_HEIGHT = 110;

/** "Level 8, quality 20%" from what the build sets. */
export function gemLevel(gem: LinkGem): string | null {
  const parts = [
    gem.level != null ? `Level ${gem.level}` : null,
    gem.quality ? `quality ${gem.quality}%` : null,
  ].filter(Boolean);
  return parts.length ? parts.join(', ') : null;
}

export type Block =
  | { kind: 'heading'; text: string }
  | { kind: 'groups'; slot: string | null; groups: LinkGroup[] };

/**
 * The groups in the build's own order, which is the order to socket them in. A label without gems
 * heads the next part, and groups one after another in the same slot share its name.
 */
export function blocks(groups: LinkGroup[]): Block[] {
  const out: Block[] = [];
  for (const g of groups) {
    if (g.gems.length === 0) {
      if (g.label) out.push({ kind: 'heading', text: g.label });
      continue;
    }
    const last = out.at(-1);
    if (last?.kind === 'groups' && last.slot !== null && last.slot === g.slot) last.groups.push(g);
    else out.push({ kind: 'groups', slot: g.slot, groups: [g] });
  }
  return out;
}

const SEP = { '--tint': 'var(--color-currency)' } as React.CSSProperties;

/**
 * The line drawing of a group the way build sites show it: each skill starts a branch, and the
 * supports after it hang from it, the last one with "└".
 */
export function connectors(gems: LinkGem[]): (string | null)[] {
  return gems.map((g, i) => {
    if (!g.support) return null;
    const next = gems[i + 1];
    return !next || !next.support ? '└' : '├';
  });
}

export interface NoteSpan {
  text: string;
  /** Null is the window's own text colour. */
  color: string | null;
}

// PoB's ^0 to ^9 come from its graphics engine. ^7 is its normal text, so it's ours too.
const DIGIT_COLORS = ['#000000', '#ff0000', '#00ff00', '#0000ff', '#ffff00', '#ff00ff', '#00ffff', null, '#b3b3b3', '#666666'];

/** A colour too dark for the black ground falls back to the normal text. */
function readable(hex: string | null): string | null {
  if (!hex) return null;
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.2 ? null : hex;
}

/**
 * The notes as PoB draws them: its colour codes carry on until the next one, and the author's
 * line breaks stay. Any run of blank lines is one paragraph break.
 */
export function parseNotes(raw: string): NoteSpan[][][] {
  const lines: NoteSpan[][] = [[]];
  let color: string | null = null;
  let text = '';
  const flush = (): void => {
    if (text) lines[lines.length - 1].push({ text, color: readable(color) });
    text = '';
  };
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    const hex = raw.slice(i + 2, i + 8);
    if (ch === '^' && /[0-9]/.test(raw[i + 1] ?? '')) {
      flush();
      color = DIGIT_COLORS[Number(raw[i + 1])];
      i += 1;
    } else if (ch === '^' && raw[i + 1] === 'x' && /^[0-9a-fA-F]{6}$/.test(hex)) {
      flush();
      color = `#${hex.toLowerCase()}`;
      i += 7;
    } else if (ch === '\n') {
      flush();
      lines.push([]);
    } else {
      text += ch;
    }
  }
  flush();
  const paragraphs: NoteSpan[][][] = [];
  let current: NoteSpan[][] = [];
  for (const line of lines) {
    if (line.every((s) => !s.text.trim())) {
      if (current.length) paragraphs.push(current);
      current = [];
    } else {
      current.push(line);
    }
  }
  if (current.length) paragraphs.push(current);
  return paragraphs;
}

function Notes({ notes }: { notes: string | null | undefined }): React.JSX.Element {
  if (notes === undefined) return <div />;
  if (!notes) {
    return <p className="font-body text-[13px] text-label italic">The build's author wrote no notes.</p>;
  }
  return (
    <div className="flex flex-col gap-2 select-text">
      {parseNotes(notes).map((paragraph, pi) => (
        <p key={pi} className="font-body text-[13px] leading-snug whitespace-pre-wrap text-plain">
          {paragraph.map((line, li) => (
            <span key={li}>
              {li > 0 && <br />}
              {line.map((s, si) =>
                s.color ? (
                  <span key={si} style={{ color: s.color }}>
                    {s.text}
                  </span>
                ) : (
                  s.text
                ),
              )}
            </span>
          ))}
        </p>
      ))}
    </div>
  );
}

type Tab = 'gems' | 'items' | 'notes';

const TABS: { value: Tab; label: string }[] = [
  { value: 'gems', label: 'Gems' },
  { value: 'items', label: 'Items' },
  { value: 'notes', label: 'Notes' },
];

const GEAR_CARD_ID = 'gear-card';


/** The stage's gear by slot. Hovering or focusing one opens the item as the game shows it. */
function GearList({ gear }: { gear: Links['gear'] }): React.JSX.Element {
  const box = useRef<HTMLDivElement>(null);
  const card = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState<{ i: number; el: HTMLElement } | null>(null);

  // Under the row, or above it when the card would run past the window's bottom. Placed after
  // layout, since an item's card is as tall as its mods.
  useLayoutEffect(() => {
    const b = box.current;
    const c = card.current;
    if (!shown || !b || !c) return;
    const line = shown.el.getBoundingClientRect();
    const top = line.top - b.getBoundingClientRect().top + b.scrollTop;
    const below = top + line.height + 4;
    const fits = below + c.offsetHeight <= b.scrollTop + b.clientHeight;
    c.style.top = `${fits ? below : Math.max(b.scrollTop, top - c.offsetHeight - 4)}px`;
  }, [shown]);

  useEffect(() => {
    if (!isTauri() && new URLSearchParams(window.location.search).has('hover')) {
      box.current?.querySelector<HTMLElement>('[role="button"]')?.focus();
    }
  }, []);

  const show = (i: number | null, el: HTMLElement | null): void => setShown(i === null || !el ? null : { i, el });
  return (
    <div ref={box} className="chat-scroll relative min-h-0 flex-1 overflow-y-auto px-3 py-3">
      {gear.length === 0 ? (
        <p className="font-body text-[13px] text-label italic">The guide has no gear for this stage.</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {gear.map((g, i) => {
            const { name, base } = itemTitle(g.item);
            return (
              <li key={`${g.slot}-${i}`} className="flex flex-col">
                <span className="font-body text-[11px] text-label">{g.slot}</span>
                <span
                  role="button"
                  tabIndex={0}
                  aria-describedby={shown?.i === i ? GEAR_CARD_ID : undefined}
                  className={`min-w-0 cursor-default truncate font-caps text-[14px] hover:bg-white/6 focus-visible:bg-white/6 focus-visible:outline-none ${RARITY[g.item.rarity]}`}
                  onMouseEnter={(e) => show(i, e.currentTarget)}
                  onMouseLeave={() => show(null, null)}
                  onFocus={(e) => show(i, e.currentTarget)}
                  onBlur={() => show(null, null)}
                >
                  {name}
                  {base && <span className="text-[13px]">, {base}</span>}
                </span>
              </li>
            );
          })}
        </ul>
      )}
      {shown && gear[shown.i] && (
        <div
          ref={card}
          id={GEAR_CARD_ID}
          role="tooltip"
          className="pointer-events-none absolute right-3 left-3 z-10 border border-edge bg-[#050505] px-3 py-2"
        >
          <ItemText item={gear[shown.i].item} />
        </div>
      )}
    </div>
  );
}

function GemCard({ gem, top }: { gem: LinkGem; top: number }): React.JSX.Element {
  const level = gemLevel(gem);
  return (
    <div
      id={CARD_ID}
      role="tooltip"
      className="pointer-events-none absolute right-3 left-3 z-10 flex flex-col gap-0.5 border border-edge bg-[#050505] px-3 py-2"
      style={{ top }}
    >
      <p className={`font-caps text-[15px] leading-snug ${COLOR[gem.color]}`}>{gem.name}</p>
      <p className="font-caps text-[13px] text-label">
        {[gem.granted ? 'Granted skill' : gem.support ? 'Support gem' : 'Skill gem', level]
          .filter(Boolean)
          .join(' · ')}
      </p>
      {gem.requiredLevel != null && gem.requiredLevel > 1 && (
        <p className="font-caps text-[13px] text-plain">Needs level {gem.requiredLevel}</p>
      )}
      {gem.get && <p className="font-body text-[13px] leading-snug text-plain italic">{gem.get}</p>}
    </div>
  );
}

function GemLine({
  gem,
  connector,
  shown,
  onHover,
}: {
  gem: LinkGem;
  connector: string | null;
  shown: boolean;
  onHover: (gem: LinkGem | null, el: HTMLElement | null) => void;
}): React.JSX.Element {
  return (
    <li className="flex items-baseline gap-1.5">
      {connector && (
        <span aria-hidden="true" className="pl-1 font-body text-[13px] text-label">
          {connector}
        </span>
      )}
      <span
        role="button"
        tabIndex={0}
        aria-describedby={shown ? CARD_ID : undefined}
        className={`min-w-0 cursor-default truncate font-caps hover:bg-white/6 focus-visible:bg-white/6 focus-visible:outline-none ${
          gem.support ? 'text-[13px]' : 'text-[14px]'
        } ${COLOR[gem.color]}`}
        onMouseEnter={(e) => onHover(gem, e.currentTarget)}
        onMouseLeave={() => onHover(null, null)}
        onFocus={(e) => onHover(gem, e.currentTarget)}
        onBlur={() => onHover(null, null)}
      >
        {gem.name}
      </span>
    </li>
  );
}

export function GemsApp(): React.JSX.Element {
  const [links, setLinks] = useState<Links | null>(null);
  const [hover, setHover] = useState<{ gem: LinkGem; top: number } | null>(null);
  const [tab, setTab] = useState<Tab>(() => {
    const q = isTauri() ? null : new URLSearchParams(window.location.search);
    return q?.has('notes') ? 'notes' : q?.has('items') ? 'items' : 'gems';
  });
  const [notes, setNotes] = useState<string | null | undefined>(undefined);
  const body = useRef<HTMLDivElement>(null);

  const load = useCallback((set: number | null) => {
    setHover(null);
    void getLinks(set).then(setLinks);
    void getNotes().then(setNotes);
  }, []);

  useEffect(() => {
    void getLinks(null).then(setLinks);
    void getNotes().then(setNotes);
    const off = isTauri() ? onGemsOpened(() => load(null)) : null;
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') void closeGems();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      void off?.then((f) => f());
      window.removeEventListener('keydown', onKey);
    };
  }, [load]);

  // The browser mock opens a card with ?hover, for screenshots.
  useEffect(() => {
    if (isTauri() || !links || !new URLSearchParams(window.location.search).has('hover')) return;
    body.current?.querySelectorAll<HTMLElement>('[role="button"]')[1]?.focus();
  }, [links]);

  const onHover = (gem: LinkGem | null, el: HTMLElement | null): void => {
    const box = body.current;
    if (!gem || !el || !box) {
      setHover(null);
      return;
    }
    // Under the line, or above it when the card would run past the window's bottom.
    const line = el.getBoundingClientRect();
    const top = line.top - box.getBoundingClientRect().top + box.scrollTop;
    const below = top + line.height + 4;
    const room = box.scrollTop + box.clientHeight - below;
    setHover({ gem, top: room < CARD_HEIGHT ? Math.max(0, top - CARD_HEIGHT) : below });
  };

  return (
    <div className="flex h-full flex-col border border-edge bg-black/90">
      <header
        data-tauri-drag-region
        className="flex h-8 shrink-0 cursor-move items-center gap-2 border-b border-edge bg-black pr-1 pl-3"
      >
        <span data-tauri-drag-region className="font-caps text-[13px] text-normal">
          <Brand />
        </span>
        <span data-tauri-drag-region className="font-caps text-[13px] text-label">
          Gems
        </span>
        <Button
          className="ml-auto grid size-6 cursor-default place-items-center p-0"
          aria-label="Close the gems"
          onClick={() => void closeGems()}
        >
          <svg viewBox="0 0 12 12" className="size-3" aria-hidden="true">
            <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" fill="none" stroke="currentColor" strokeWidth="1.4" />
          </svg>
        </Button>
      </header>
      {links && (
        <div className="flex shrink-0 gap-2 px-3 pt-3">
          <Choice value={tab} options={TABS} onChange={setTab} />
        </div>
      )}
      {!links ? (
        <p className="px-4 py-4 font-body text-[13px] text-label italic">
          Follow a guide or import your build in the settings to see its gems here.
        </p>
      ) : tab === 'notes' ? (
        <div className="chat-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3">
          <Notes notes={notes} />
        </div>
      ) : (
        <>
          <div className="shrink-0 px-3 pt-3">
            <Select
              value={String(links.set)}
              label="Stage"
              className="w-full"
              options={links.sets.map((title, i) => ({ value: String(i), label: title }))}
              onChange={(v) => load(Number(v))}
            />
          </div>
          {tab === 'items' ? (
            <GearList gear={links.gear} />
          ) : (
            <div ref={body} className="chat-scroll relative min-h-0 flex-1 overflow-y-auto px-3 py-3">
              {links.groups.length === 0 && (
                <p className="font-body text-[13px] text-label italic">No gems in this stage.</p>
              )}
              {blocks(links.groups).map((b, bi, all) => {
                const prev = all[bi - 1];
                // Slots and headings get a separator. Groups without a slot only a gap.
                const sep = bi > 0 && (b.kind === 'heading' || b.slot !== null || (prev.kind === 'groups' && prev.slot !== null));
                if (b.kind === 'heading') {
                  return (
                    <div key={bi}>
                      {sep && <div className="tooltip-sep my-2.5" style={SEP} />}
                      <h2 className="font-caps text-[13px] text-value">{b.text}</h2>
                    </div>
                  );
                }
                return (
                <section
                  key={bi}
                  className={`lazy-row flex flex-col [contain-intrinsic-size:auto_90px] ${bi > 0 && !sep ? 'mt-2' : ''}`}
                  aria-label={b.slot ?? b.groups[0].gems[0]?.name}
                >
                  {sep && <div className="tooltip-sep my-2.5" style={SEP} />}
                  {b.slot && <p className="font-body text-[11px] text-label">{b.slot}</p>}
                  {b.groups.map((g, gi) => {
                    const lines = connectors(g.gems);
                    return (
                      <ul key={gi} className={`flex flex-col ${gi > 0 ? 'mt-1.5' : ''}`}>
                        {g.label && <li className="font-body text-[12px] leading-snug text-plain italic">{g.label}</li>}
                        {g.gems.map((gem, i) => (
                          <GemLine
                            key={`${gem.name}-${i}`}
                            gem={gem}
                            connector={lines[i]}
                            shown={hover?.gem === gem}
                            onHover={onHover}
                          />
                        ))}
                      </ul>
                    );
                  })}
                </section>
                );
              })}
              {hover && <GemCard gem={hover.gem} top={hover.top} />}
            </div>
          )}
        </>
      )}
    </div>
  );
}
