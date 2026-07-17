import { replayPause, replayResume, replaySetSpeed, replayStep } from '../ipc/commands';
import type { SpeedMultiplier } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';

const SPEEDS: readonly SpeedMultiplier[] = ['X1', 'X2', 'X5', 'X20', 'X100'];
const speedLabel = (s: SpeedMultiplier): string => `${s.slice(1)}×`;

export function ReplayControls(): React.JSX.Element | null {
  const replay = useAppStore((s) => s.replay);
  const mode = useAppStore((s) => s.source.mode);

  if (mode !== 'replay') return null;

  const pct = replay.total > 0 ? Math.round((replay.position / replay.total) * 100) : 0;

  return (
    <div className="mt-2 flex flex-col gap-1 border-t border-slate-700/50 pt-2">
      <div className="flex items-center gap-1">
        <button
          className="rounded bg-slate-700 px-2 py-0.5 text-xs hover:bg-slate-600"
          onClick={replay.paused ? replayResume : replayPause}
        >
          {replay.paused ? '▶ Play' : '⏸ Pause'}
        </button>
        <button
          className="rounded bg-slate-700 px-2 py-0.5 text-xs hover:bg-slate-600 disabled:opacity-40"
          onClick={replayStep}
          disabled={!replay.paused}
        >
          ⏭ Step
        </button>
        <div className="ml-auto flex gap-0.5">
          {SPEEDS.map((s) => (
            <button
              key={s}
              className={[
                'rounded px-1.5 py-0.5 text-xs',
                replay.speed === s ? 'bg-cyan-600 text-white' : 'bg-slate-700 hover:bg-slate-600',
              ].join(' ')}
              onClick={() => replaySetSpeed(s)}
            >
              {speedLabel(s)}
            </button>
          ))}
        </div>
      </div>
      <div className="h-1 w-full overflow-hidden rounded bg-slate-800">
        <div className="h-full bg-cyan-500" style={{ width: `${pct}%` }} />
      </div>
      <div className="text-right text-[10px] tabular-nums text-slate-400">
        {replay.position}/{replay.total}
      </div>
    </div>
  );
}
