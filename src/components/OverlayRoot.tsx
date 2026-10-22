import { useEffect, useLayoutEffect, useRef, useState } from 'react';

import { fitOverlay } from '../ipc/commands';
import { holdLabel, shortcutLabel } from './HotkeySection';
import { useAppStore } from '../store/useAppStore';
import { CompactPanel, RouteNav, StatusPanel } from './StatusPanel';
import { TooltipHeader } from './Tooltip';

const NOTICE_MS = 5000;
const FULL_WIDTH = 404;
const COMPACT_WIDTH = 340;

// True once `seconds` pass with the same `id`; any new id, or 0 seconds, shows it again.
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

// Shown once when poebo starts without the game in front, instead of the route.
function StartupNotice(): React.JSX.Element {
  const holdKey = useAppStore((s) => s.hotkeys.menuHold);
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
      <TooltipHeader tint="var(--color-currency)" title="poebo" subtitle="Running in the background" />
      <p className="px-4 pt-2 pb-3 font-body text-[13px] leading-snug text-label italic">
        It appears over Path of Exile while the game is focused. Hold{' '}
        {holdLabel(holdKey)} in the game for the menu.
      </p>
    </div>
  );
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
  // Coming back to the game counts as a change too, so an Alt-Tab shows the route again.
  const faded = useFaded(`${edge}|${zoneId}|${gameFocused}`, grabbed ? 0 : fadeAfter);
  const visible = ((gameFocused && routeEnabled) || grabbed) && !faded;
  const [shownOnce, setShownOnce] = useState(false);
  if (visible && !shownOnce) setShownOnce(true);

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

  // Hiding is instant to match the game window.
  return (
    <div ref={box} className="inline-block p-2">
      {!shownOnce && <StartupNotice />}
      <div
        hidden={!shownOnce}
        className={[
          grabbed ? 'pointer-events-auto outline outline-1 outline-offset-2 outline-white/35' : 'pointer-events-none',
          visible
            ? 'opacity-100 transition-opacity duration-150'
            : faded
              ? 'opacity-0 transition-opacity duration-700'
              : 'opacity-0',
        ].join(' ')}
        style={{
          backgroundColor: `rgb(0 0 0 / ${opacity})`,
          zoom: textScale,
          width: compact ? COMPACT_WIDTH : FULL_WIDTH,
        }}
      >
        {grabbed && (
          <>
            <div
              data-tauri-drag-region
              className="flex cursor-move items-baseline justify-between border-b border-edge px-3 pt-1.5 pb-1"
            >
              <span data-tauri-drag-region className="font-caps text-[13px] text-normal">
                poebo
              </span>
              <span data-tauri-drag-region className="text-[11px] text-label">
                Drag to move, {shortcutLabel(hotkeys.moveOverlay)} to lock.
                Hold {holdLabel(hotkeys.menuHold)} for the menu.
              </span>
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
