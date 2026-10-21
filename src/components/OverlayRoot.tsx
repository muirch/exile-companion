import { useEffect, useState } from 'react';

import { useAppStore } from '../store/useAppStore';
import { ReplayControls } from './ReplayControls';
import { SettingsPanel } from './SettingsPanel';
import { StatusPanel } from './StatusPanel';
import { TooltipHeader } from './Tooltip';

const NOTICE_MS = 5000;

// Shown once when poebo starts without the game in front, instead of the route.
function StartupNotice(): React.JSX.Element {
  const [shown, setShown] = useState(true);
  useEffect(() => {
    const t = setTimeout(() => setShown(false), NOTICE_MS);
    return () => clearTimeout(t);
  }, []);

  return (
    <div
      role="status"
      className={[
        'tooltip pointer-events-none absolute inset-x-0 bottom-2 mx-auto w-72 bg-black/85 transition-opacity duration-700',
        shown ? 'opacity-100' : 'opacity-0',
      ].join(' ')}
    >
      <TooltipHeader tint="var(--color-currency)" title="poebo" subtitle="Running in the background" />
      <p className="px-4 pt-2 pb-3 font-body text-[13px] leading-snug text-label italic">
        It appears over Path of Exile while the game is focused. Ctrl+Alt+Space unlocks it.
      </p>
    </div>
  );
}

export function OverlayRoot(): React.JSX.Element {
  const grabbed = useAppStore((s) => s.ui.grabbed);
  const opacity = useAppStore((s) => s.ui.opacity);
  const textScale = useAppStore((s) => s.ui.textScale);
  const gameFocused = useAppStore((s) => s.ui.gameFocused);
  const visible = gameFocused || grabbed;
  const [shownOnce, setShownOnce] = useState(false);
  if (visible && !shownOnce) setShownOnce(true);

  // Anchored to the bottom of the window: unlocking adds controls above the route, so the route
  // itself stays where the player put it. Hiding is instant to match the game window.
  return (
    <div className="relative flex h-full flex-col justify-end p-2">
      {!shownOnce && <StartupNotice />}
      <div
        className={[
          grabbed ? 'pointer-events-auto outline outline-1 outline-offset-2 outline-currency/60' : 'pointer-events-none',
          visible ? 'opacity-100 transition-opacity duration-150' : 'opacity-0',
        ].join(' ')}
        style={{ backgroundColor: `rgb(0 0 0 / ${opacity})`, zoom: textScale }}
      >
        {grabbed && (
          <>
            <div
              data-tauri-drag-region
              className="flex cursor-move items-baseline justify-between border-b border-frame-dark px-3 pt-1.5 pb-1"
            >
              <span data-tauri-drag-region className="font-caps text-[13px] text-currency">
                poebo
              </span>
              <span data-tauri-drag-region className="text-[11px] text-label">
                Drag to move. Ctrl+Alt+Space locks it.
              </span>
            </div>
            <div className="border-b border-frame-dark px-3 py-2.5">
              <SettingsPanel />
              <ReplayControls />
            </div>
          </>
        )}
        <StatusPanel />
      </div>
    </div>
  );
}
