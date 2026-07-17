import { useAppStore } from '../store/useAppStore';

const actLabel = (act: number | null): string => (act == null ? '-' : `Act ${act}`);

function ActZoneDisplay(): React.JSX.Element {
  const zone = useAppStore((s) => s.campaign.zone);
  const act = useAppStore((s) => s.campaign.act);
  const location = useAppStore((s) => s.campaign.location_kind);

  return (
    <div className="min-w-0">
      <div className="text-[10px] uppercase tracking-wider text-cyan-300/70">{actLabel(act)}</div>
      <div className="truncate text-lg font-semibold leading-tight">
        {zone?.name ?? 'Waiting for log...'}
      </div>
      {(location === 'town' || location === 'hideout') && (
        <div className="text-[10px] capitalize text-slate-400">{location}</div>
      )}
    </div>
  );
}

function LevelBadge(): React.JSX.Element {
  const level = useAppStore((s) => s.campaign.level);
  const klass = useAppStore((s) => s.campaign.class);

  return (
    <div className="flex shrink-0 flex-col items-end">
      <div className="font-mono text-2xl font-bold leading-none tabular-nums">
        {level > 0 ? level : '-'}
      </div>
      <div className="text-[10px] text-slate-400">{klass ?? 'level'}</div>
    </div>
  );
}

function NextObjective(): React.JSX.Element {
  // Placeholder render target; Phase 2 fills this from the PoB build + campaign knowledge base.
  return (
    <div className="rounded-md bg-slate-800/40 px-2 py-1 text-xs text-slate-300">
      Next objective - coming in Phase 2
    </div>
  );
}

export function StatusPanel(): React.JSX.Element {
  const deaths = useAppStore((s) => s.campaign.deaths);

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-start justify-between gap-3">
        <ActZoneDisplay />
        <LevelBadge />
      </div>
      <NextObjective />
      {deaths > 0 && <div className="text-xs text-rose-400/80">Deaths: {deaths}</div>}
    </div>
  );
}
