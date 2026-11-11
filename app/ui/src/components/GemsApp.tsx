import { useCallback, useEffect, useRef, useState } from 'react';

import { closeGems, getLinks, getNotes } from '../ipc/commands';
import { isTauri } from '../ipc/env';
import { onGemsOpened } from '../ipc/events';
import type { GemColor, LinkGem, LinkGroup, Links } from '../ipc/types';
import { Button, Choice, Select } from '../kit';
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

/** Link groups by item slot, in the order the build first names each slot. */
export function bySlot(groups: LinkGroup[]): { slot: string | null; groups: LinkGroup[] }[] {
  const out: { slot: string | null; groups: LinkGroup[] }[] = [];
  for (const g of groups) {
    const at = out.find((s) => s.slot === g.slot && g.slot !== null);
    if (at) at.groups.push(g);
    else out.push({ slot: g.slot, groups: [g] });
  }
  return out;
}

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

/** The notes in paragraphs. A line like "- Act 1 -" is how authors head a part. */
export function noteBlocks(notes: string): { heading: boolean; text: string }[] {
  return notes
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .filter(Boolean)
    .map((p) => {
      const heading = /^-+\s*[^\n]+?\s*-+$/.test(p);
      return { heading, text: heading ? p.replace(/^-+\s*|\s*-+$/g, '') : p };
    });
}

function Notes({ notes }: { notes: string | null | undefined }): React.JSX.Element {
  if (notes === undefined) return <div />;
  if (!notes) {
    return <p className="font-body text-[13px] text-label italic">The build's author wrote no notes.</p>;
  }
  return (
    <div className="flex flex-col gap-2 select-text">
      {noteBlocks(notes).map((b, i) =>
        b.heading ? (
          <h2 key={i} className={`font-caps text-[14px] text-value ${i > 0 ? 'mt-2' : ''}`}>
            {b.text}
          </h2>
        ) : (
          <p key={i} className="font-body text-[13px] leading-snug whitespace-pre-line text-plain">
            {b.text}
          </p>
        ),
      )}
    </div>
  );
}

const TABS = [
  { value: 'gems' as const, label: 'Gems' },
  { value: 'notes' as const, label: 'Notes' },
];

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
        {[gem.support ? 'Support gem' : 'Skill gem', level].filter(Boolean).join(' · ')}
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
  const [tab, setTab] = useState<'gems' | 'notes'>(() =>
    !isTauri() && new URLSearchParams(window.location.search).has('notes') ? 'notes' : 'gems',
  );
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
          <div ref={body} className="chat-scroll relative min-h-0 flex-1 overflow-y-auto px-3 py-3">
            {links.groups.length === 0 && (
              <p className="font-body text-[13px] text-label italic">No gems in this stage.</p>
            )}
            {bySlot(links.groups).map((s, si) => (
              <section key={si} className="flex flex-col" aria-label={s.slot ?? 'No slot'}>
                {si > 0 && (
                  <div className="tooltip-sep my-2.5" style={{ '--tint': 'var(--color-currency)' } as React.CSSProperties} />
                )}
                <p className="font-body text-[11px] text-label">{s.slot ?? 'No slot'}</p>
                {s.groups.map((g, gi) => {
                  const lines = connectors(g.gems);
                  return (
                    <ul key={gi} className={`flex flex-col ${gi > 0 ? 'mt-1.5' : ''}`}>
                      {g.label && <li className="font-body text-[11px] text-label italic">{g.label}</li>}
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
            ))}
            {hover && <GemCard gem={hover.gem} top={hover.top} />}
          </div>
        </>
      )}
    </div>
  );
}
