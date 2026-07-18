import type { ObjectiveKind } from '../ipc/types';
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

// Accent colour per objective kind so the eye can tell them apart at a glance.
const KIND_ACCENT: Record<ObjectiveKind, string> = {
  quest_reward: 'border-emerald-400/60 bg-emerald-500/10 text-emerald-200',
  proceed_to_zone: 'border-cyan-400/60 bg-cyan-500/10 text-cyan-200',
  lab_trial: 'border-amber-400/60 bg-amber-500/10 text-amber-200',
  build_spec: 'border-indigo-400/60 bg-indigo-500/10 text-indigo-200',
  idle: 'border-slate-600/60 bg-slate-800/40 text-slate-300',
};

function NextObjective(): React.JSX.Element {
  const guidance = useAppStore((s) => s.guidance);

  if (!guidance) {
    return (
      <div className="rounded-md border border-slate-700/50 bg-slate-800/40 px-2 py-1 text-xs text-slate-400">
        Waiting for the campaign to start...
      </div>
    );
  }

  return (
    <div className={`rounded-md border px-2 py-1 ${KIND_ACCENT[guidance.kind]}`}>
      <div className="text-xs font-semibold leading-snug">{guidance.headline}</div>
      {guidance.detail && (
        <div className="text-[10px] leading-tight opacity-80">{guidance.detail}</div>
      )}
    </div>
  );
}

function BuildLine(): React.JSX.Element | null {
  const build = useAppStore((s) => s.build);
  if (!build) return null;

  const klass = [build.class_name, build.ascend_class_name].filter(Boolean).join(' / ');
  const parts = [klass || 'Build', build.active_spec_title, build.main_skill].filter(Boolean);

  return (
    <div className="flex items-center gap-1 text-[10px] text-slate-400">
      <span className="rounded bg-slate-700/60 px-1 py-px uppercase tracking-wider text-slate-300">
        Build
      </span>
      <span className="truncate">{parts.join(' - ')}</span>
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
      <BuildLine />
      {deaths > 0 && <div className="text-xs text-rose-400/80">Deaths: {deaths}</div>}
    </div>
  );
}
