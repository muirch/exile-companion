import { replayPause, replayResume, replaySetSpeed, replayStep } from '../ipc/commands';
import type { SpeedMultiplier } from '../ipc/types';
import { Button, Choice } from '../kit';
import { useAppStore } from '../store/useAppStore';

const SPEEDS = (['X1', 'X2', 'X5', 'X20', 'X100'] as const).map((s: SpeedMultiplier) => ({
  value: s,
  label: `${s.slice(1)}x`,
  literal: true,
}));

export function ReplayControls(): React.JSX.Element | null {
  const replay = useAppStore((s) => s.replay);
  const mode = useAppStore((s) => s.source.mode);

  if (mode !== 'replay') return null;

  const pct = replay.total > 0 ? Math.round((replay.position / replay.total) * 100) : 0;

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-1.5">
        <Button onClick={() => void (replay.paused ? replayResume() : replayPause())}>
          {replay.paused ? 'Play' : 'Pause'}
        </Button>
        <Button onClick={() => void replayStep()} disabled={!replay.paused}>
          Step
        </Button>
        <div className="ml-auto flex gap-1">
          <Choice value={replay.speed} options={SPEEDS} onChange={(s) => void replaySetSpeed(s)} />
        </div>
      </div>
      <div className="flex items-center gap-2">
        <div className="h-[3px] flex-1 bg-edge shadow-[0_0_0_1px_#000]">
          <div className="h-full bg-normal" style={{ width: `${pct}%` }} />
        </div>
        <span className="text-[11px] text-label">
          {replay.position}/{replay.total} lines
        </span>
      </div>
    </div>
  );
}
