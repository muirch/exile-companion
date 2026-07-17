import { useAppStore } from '../store/useAppStore';
import { ReplayControls } from './ReplayControls';
import { SettingsPanel } from './SettingsPanel';
import { StatusPanel } from './StatusPanel';

/**
 * The overlay panel. Transparent window; only this panel paints, at the user's opacity. When
 * "grabbed" (toggled by the global hotkey) it becomes interactive, shows a drag handle + control
 * strips, and outlines itself; otherwise it's click-through so the game receives the mouse.
 */
export function OverlayRoot(): React.JSX.Element {
  const grabbed = useAppStore((s) => s.ui.grabbed);
  const opacity = useAppStore((s) => s.ui.opacity);

  return (
    <div
      className={[
        'm-2 overflow-hidden rounded-xl text-slate-100 backdrop-blur-sm',
        grabbed ? 'pointer-events-auto ring-1 ring-cyan-400/50' : 'pointer-events-none',
      ].join(' ')}
      style={{ backgroundColor: `rgba(2, 6, 23, ${opacity})` }}
    >
      {grabbed && (
        <div
          data-tauri-drag-region
          className="flex cursor-move items-center justify-between px-3 py-1 text-[10px] uppercase tracking-wider text-cyan-300/70"
        >
          <span data-tauri-drag-region>poebo - drag</span>
          <span className="text-slate-400">Ctrl+Alt+Space to release</span>
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
  );
}
