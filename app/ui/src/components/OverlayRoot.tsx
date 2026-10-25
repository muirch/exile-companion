import { useEffect, useLayoutEffect, useRef, useState } from 'react';

import { fitOverlay, lockOverlay } from '../ipc/commands';
import { isTauri } from '../ipc/env';
import { onItemCard } from '../ipc/events';
import type { ItemCard as Card } from '../ipc/types';
import { holdLabel, shortcutLabel } from './HotkeySection';
import { useAppStore } from '../store/useAppStore';
import { ItemCard } from './ItemCard';
import { CompactPanel, RouteNav, StatusPanel } from './StatusPanel';
import { Brand, TooltipHeader } from './Tooltip';

const NOTICE_MS = 9000;
export const ITEM_CARD_MS = 12000;
const FULL_WIDTH = 404;
const COMPACT_WIDTH = 340;

// PoE scales its UI with the monitor height, not with the Windows scale factor, so the plate does
// the same: 100% text size matches the game at 1080p. screen.height is already in CSS pixels.
export function screenZoom(height: number): number {
  return Math.min(2, Math.max(0.75, height / 1080));
}

// True once `seconds` pass with the same `id`. Any new id, or 0 seconds, shows it again.
function useFaded(id: string, seconds: number): boolean {
  const [state, setState] = useState({ id, faded: false });
  if (state.id !== id) setState({ id, faded: false });
  useEffect(() => {
    if (seconds <= 0) return;
    const t = setTimeout(
      () => setState((s) => (s.id === id ? { id, faded: true } : s)),
      seconds * 1000,
    );
    return () => clearTimeout(t);
  }, [id, seconds]);
  return seconds > 0 && state.id === id && state.faded;
}

// Shown once when Exile Companion starts without the game in front, instead of the route.
function StartupNotice(): React.JSX.Element {
  const keys = useAppStore((s) => s.hotkeys);
  const compact = useAppStore((s) => s.ui.compact);
  const [shown, setShown] = useState(true);
  useEffect(() => {
    const t = setTimeout(() => setShown(false), NOTICE_MS);
    return () => clearTimeout(t);
  }, []);

  return (
    <div
      role="status"
      className={[
        'tooltip pointer-events-none w-72 bg-black/85 transition-opacity duration-700',
        shown ? 'opacity-100' : 'opacity-0',
      ].join(' ')}
    >
      <TooltipHeader
        tint="var(--color-currency)"
        title={<Brand />}
        subtitle="Running in the background"
      />
      <p className="px-4 pt-2 pb-3 font-body text-[13px] leading-snug text-plain italic">
        It appears over Path of Exile while the game is focused. Hold {holdLabel(keys.menuHold)}{' '}
        in the game for the menu. {shortcutLabel(keys.routePrev)} and{' '}
        {shortcutLabel(keys.routeNext)} step the route if it gets ahead or behind.
        {compact && ' Turn on Full layout in the menu for directions and hints.'}
      </p>
    </div>
  );
}

// The last copied item, until it has been on screen for a while. Measuring stays until the answer.
export function useItemCard(): Card | null {
  const [card, setCard] = useState<Card | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    const off = onItemCard(setCard);
    return () => void off.then((f) => f());
  }, []);
  useEffect(() => {
    if (!card || card.kind === 'measuring') return;
    const t = setTimeout(() => setCard(null), ITEM_CARD_MS);
    return () => clearTimeout(t);
  }, [card]);
  return card;
}

export function OverlayRoot(): React.JSX.Element {
  const grabbed = useAppStore((s) => s.ui.grabbed);
  const opacity = useAppStore((s) => s.ui.opacity);
  const textScale = useAppStore((s) => s.ui.textScale);
  const gameFocused = useAppStore((s) => s.ui.gameFocused);
  const routeEnabled = useAppStore((s) => s.routeEnabled);
  const hotkeys = useAppStore((s) => s.hotkeys);
  const compact = useAppStore((s) => s.ui.compact);
  const fadeAfter = useAppStore((s) => s.ui.fadeAfter);
  const edge = useAppStore((s) => s.route?.edge);
  const zoneId = useAppStore((s) => s.campaign.zone?.id);
  const peeks = useAppStore((s) => s.ui.peeks);
  // Coming back to the game and tapping the menu key count as a change too.
  const faded = useFaded(`${edge}|${zoneId}|${gameFocused}|${peeks}`, grabbed ? 0 : fadeAfter);
  const visible = ((gameFocused && routeEnabled) || grabbed) && !faded;
  const [shownOnce, setShownOnce] = useState(false);
  if (visible && !shownOnce) setShownOnce(true);
  const itemCard = useItemCard();
  const zoom = textScale * (isTauri() ? screenZoom(window.screen.height) : 1);
  const width = compact ? COMPACT_WIDTH : FULL_WIDTH;

  // The overlay has keyboard focus while it is unlocked, so Esc reaches it.
  useEffect(() => {
    if (!grabbed) return;
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') void lockOverlay();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [grabbed]);

  // The window is sized to this box and keeps its bottom center fixed, so the route stays where
  // the player put it when the text grows. Move-mode controls all go above the route for the
  // same reason: anything below it would shift the route when the overlay locks again.
  const box = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const report = (): void => {
      const r = el.getBoundingClientRect();
      void fitOverlay(Math.ceil(r.width), Math.ceil(r.height));
    };
    const ro = new ResizeObserver(report);
    ro.observe(el);
    report();
    return () => ro.disconnect();
  }, []);

  // Showing and hiding are instant to match the game window. Only the timed fade-out is animated.
  return (
    <div ref={box} className="inline-block p-2">
      {!shownOnce && <StartupNotice />}
      {itemCard && (gameFocused || grabbed) && (
        <div
          className="pointer-events-none mb-2"
          style={{ backgroundColor: `rgb(0 0 0 / ${opacity})`, zoom, width }}
        >
          <ItemCard card={itemCard} compact={compact} />
        </div>
      )}
      <div
        hidden={!shownOnce}
        className={[
          grabbed ? 'pointer-events-auto outline outline-1 outline-offset-2 outline-normal/60' : 'pointer-events-none',
          visible
            ? 'opacity-100'
            : faded
              ? 'opacity-0 transition-opacity duration-700'
              : 'opacity-0',
        ].join(' ')}
        style={{
          backgroundColor: `rgb(0 0 0 / ${opacity})`,
          zoom,
          width,
        }}
      >
        {grabbed && (
          <>
            <div
              data-tauri-drag-region
              className="flex cursor-move items-center gap-2 border-b border-edge py-1 pr-1 pl-3"
            >
              <span data-tauri-drag-region className="font-caps text-[13px] text-normal">
                <Brand />
              </span>
              <span data-tauri-drag-region className="flex-1 text-right text-[11px] text-plain">
                Drag to move. Esc or {shortcutLabel(hotkeys.moveOverlay)} locks.
              </span>
              <button className="btn cursor-default" onClick={() => void lockOverlay()}>
                Lock
              </button>
            </div>
            {routeEnabled && (
              <div className="border-b border-edge pb-1.5">
                <RouteNav />
              </div>
            )}
          </>
        )}
        {routeEnabled && (compact ? <CompactPanel /> : <StatusPanel />)}
      </div>
    </div>
  );
}
