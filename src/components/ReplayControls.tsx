import { replayPause, replayResume, replaySetSpeed, replayStep } from '../ipc/commands';
import type { SpeedMultiplier } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';

const SPEEDS: readonly SpeedMultiplier[] = ['X1', 'X2', 'X5', 'X20', 'X100'];
const speedLabel = (s: SpeedMultiplier): string => `${s.slice(1)}x`;

export function ReplayControls(): React.JSX.Element | null {
  const replay = useAppStore((s) => s.replay);
  const mode = useAppStore((s) => s.source.mode);

  if (mode !== 'replay') return null;

  const pct = replay.total > 0 ? Math.round((replay.position / replay.total) * 100) : 0;

  return (
    <div className="mt-2.5 flex flex-col gap-1.5">
      <div className="flex items-center gap-1.5">
        <button className="btn" onClick={replay.paused ? replayResume : replayPause}>
          {replay.paused ? 'Play' : 'Pause'}
        </button>
        <button className="btn" onClick={replayStep} disabled={!replay.paused}>
          Step
        </button>
        <div className="ml-auto flex gap-1">
          {SPEEDS.map((s) => (
            <button
              key={s}
              className="btn font-body"
              aria-pressed={replay.speed === s}
              onClick={() => replaySetSpeed(s)}
            >
              {speedLabel(s)}
            </button>
          ))}
        </div>
      </div>
      <div className="flex items-center gap-2">
        <div className="h-[3px] flex-1 bg-frame-dark shadow-[0_0_0_1px_#000]">
          <div className="h-full bg-currency" style={{ width: `${pct}%` }} />
        </div>
        <span className="text-[11px] text-label">
          {replay.position}/{replay.total} lines
        </span>
      </div>
    </div>
  );
}
