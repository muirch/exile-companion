import { useEffect, useState } from 'react';

import { useAppStore } from '../store/useAppStore';
import { ReplayControls } from './ReplayControls';
import { SettingsPanel } from './SettingsPanel';
import { StatusPanel } from './StatusPanel';

const TOAST_MS = 5000;

// Shown once at startup when the game isn't focused, so the player knows poebo is running.
function StartupToast(): React.JSX.Element {
  const [shown, setShown] = useState(true);
  useEffect(() => {
    const t = setTimeout(() => setShown(false), TOAST_MS);
    return () => clearTimeout(t);
  }, []);

  return (
    <div
      className={[
        'pointer-events-none m-2 rounded-xl bg-slate-950/90 px-3 py-2 text-xs text-slate-200 transition-opacity duration-700',
        shown ? 'opacity-100' : 'opacity-0',
      ].join(' ')}
    >
      <div className="font-semibold text-cyan-300">poebo is running</div>
      <div className="text-slate-400">
        It shows up over Path of Exile. Ctrl+Alt+Space to move it or open settings.
      </div>
    </div>
  );
}

export function OverlayRoot(): React.JSX.Element {
  const grabbed = useAppStore((s) => s.ui.grabbed);
  const opacity = useAppStore((s) => s.ui.opacity);
  const gameFocused = useAppStore((s) => s.ui.gameFocused);
  const [seenGame, setSeenGame] = useState(gameFocused);
  if (gameFocused && !seenGame) setSeenGame(true);

  const visible = gameFocused || grabbed;

  return (
    <div className="relative">
      {!seenGame && (
        <div className="absolute inset-x-0 top-0">
          <StartupToast />
        </div>
      )}
      <div
        className={[
          'm-2 overflow-hidden rounded-xl text-slate-100 backdrop-blur-sm transition-opacity duration-300',
          grabbed ? 'pointer-events-auto ring-1 ring-cyan-400/50' : 'pointer-events-none',
          visible ? 'opacity-100' : 'opacity-0',
        ].join(' ')}
        style={{ backgroundColor: `rgba(2, 6, 23, ${opacity})` }}
      >
        {grabbed && (
          <div
            data-tauri-drag-region
            className="flex cursor-move items-center justify-between px-3 py-1 text-[10px] uppercase tracking-wider text-cyan-300/70"
          >
            <span data-tauri-drag-region>poebo</span>
            <span className="text-slate-400">drag to move · Ctrl+Alt+Space to lock</span>
          </div>
        )}

        <div className="px-3 pb-3 pt-2">
          <StatusPanel />
          {grabbed && (
            <>
              <ReplayControls />
              <SettingsPanel />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
