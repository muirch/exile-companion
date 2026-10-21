import { useEffect, useState } from 'react';

import { useAppStore } from '../store/useAppStore';
import { ReplayControls } from './ReplayControls';
import { SettingsPanel } from './SettingsPanel';
import { StatusPanel } from './StatusPanel';

const NOTICE_MS = 5000;

// Shown once when poebo starts without the game in front, instead of the route plate.
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
        'plate pointer-events-none absolute inset-x-0 bottom-2 mx-auto w-72 bg-umber/95 px-4 py-3 text-center transition-opacity duration-700',
        shown ? 'opacity-100' : 'opacity-0',
      ].join(' ')}
    >
      <div className="flex items-center gap-3">
        <div className="rule" />
        <p className="font-caps text-[15px] font-bold text-gold">poebo is running</p>
        <div className="rule rule-end" />
      </div>
      <p className="mt-1 text-[12px] leading-snug text-ash-dim">
        It appears over Path of Exile while the game is focused. Ctrl+Alt+Space unlocks it.
      </p>
    </div>
  );
}

export function OverlayRoot(): React.JSX.Element {
  const grabbed = useAppStore((s) => s.ui.grabbed);
  const opacity = useAppStore((s) => s.ui.opacity);
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
          'plate',
          grabbed ? 'pointer-events-auto border-bronze-bright' : 'pointer-events-none',
          visible ? 'opacity-100 transition-opacity duration-150' : 'opacity-0',
        ].join(' ')}
        style={{ backgroundColor: `rgb(14 11 8 / ${opacity})` }}
      >
        {grabbed && (
          <>
            <div
              data-tauri-drag-region
              className="flex cursor-move items-baseline justify-between border-b border-bronze/70 px-3 pt-1.5 pb-1"
            >
              <span data-tauri-drag-region className="font-caps text-[13px] font-bold text-gold">
                poebo
              </span>
              <span data-tauri-drag-region className="text-[11px] text-ash-faint">
                Drag to move. Ctrl+Alt+Space locks it.
              </span>
            </div>
            <div className="border-b border-bronze/70 px-3 py-2.5">
              <SettingsPanel />
              <ReplayControls />
            </div>
          </>
        )}
        <div className="px-4 pt-3 pb-3.5">
          <StatusPanel />
        </div>
      </div>
    </div>
  );
}
