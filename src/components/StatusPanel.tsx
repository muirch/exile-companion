import { useAppStore } from '../store/useAppStore';
import { RoutePanel } from './RoutePanel';

function ZoneHeader(): React.JSX.Element {
  const zone = useAppStore((s) => s.campaign.zone);
  const act = useAppStore((s) => s.campaign.act);
  const level = useAppStore((s) => s.campaign.level);
  const klass = useAppStore((s) => s.campaign.class);
  const route = useAppStore((s) => s.route);

  const facts = [
    act != null ? `Act ${act}` : null,
    level > 0 ? `Level ${level}${klass ? ` ${klass}` : ''}` : null,
    route ? `Step ${route.edge + 1} of ${route.edge_count}` : null,
  ].filter(Boolean);

  return (
    <header className="flex flex-col items-center gap-1">
      <div className="flex w-full items-center gap-3">
        <div className="rule" />
        <h1 className="max-w-[75%] truncate font-caps text-[20px] leading-none font-bold tracking-[0.03em] text-gold">
          {zone?.name ?? 'Waiting for the game'}
        </h1>
        <div className="rule rule-end" />
      </div>
      {facts.length > 0 && <p className="text-[12px] text-ash-dim">{facts.join(' · ')}</p>}
    </header>
  );
}

// The route covers where to go; from the old guidance only the build's gem claims add anything.
function GemClaim(): React.JSX.Element | null {
  const guidance = useAppStore((s) => s.guidance);
  if (guidance?.kind !== 'quest_reward') return null;

  return (
    <>
      <div className="separator" />
      <p className="text-[13px] leading-snug text-gem">
        {guidance.headline}
        {guidance.detail && <span className="text-ash-dim"> · {guidance.detail}</span>}
      </p>
    </>
  );
}

function BuildLine(): React.JSX.Element | null {
  const build = useAppStore((s) => s.build);
  const deaths = useAppStore((s) => s.campaign.deaths);
  if (!build && deaths === 0) return null;

  const klass = build ? [build.class_name, build.ascend_class_name].filter(Boolean).join(' ') : '';
  const parts = build ? [klass || 'Build', build.active_spec_title, build.main_skill] : [];
  if (deaths > 0) parts.push(`${deaths} ${deaths === 1 ? 'death' : 'deaths'} this session`);

  return <p className="truncate text-center text-[11px] text-ash-faint">{parts.filter(Boolean).join(' · ')}</p>;
}

function NoLogHint(): React.JSX.Element | null {
  const mode = useAppStore((s) => s.source.mode);
  const error = useAppStore((s) => s.source.error);
  if (mode !== 'idle') return null;
  return (
    <p className="text-center text-[13px] leading-snug text-trial">
      {error ?? 'Client.txt not found.'} Unlock with Ctrl+Alt+Space, then pick logs/Client.txt in
      the game folder.
    </p>
  );
}

export function StatusPanel(): React.JSX.Element {
  const grabbed = useAppStore((s) => s.ui.grabbed);

  return (
    <div className="flex flex-col gap-2.5">
      <ZoneHeader />
      <NoLogHint />
      <div className="separator" />
      <RoutePanel />
      <GemClaim />
      {grabbed && <BuildLine />}
    </div>
  );
}
