import { routeNext, routePrev } from '../ipc/commands';
import { useAppStore } from '../store/useAppStore';
import { holdLabel, shortcutLabel } from './HotkeySection';
import { HereSteps, PassThrough, ThenStep, hasActions } from './RoutePanel';
import { TooltipHeader, TooltipSep } from './Tooltip';

// Sand (the currency item's tint) for every header; towns are told apart by the subtitle.
const TINT = 'var(--color-currency)';

function Prop({ label, value }: { label: string; value: string | number }): React.JSX.Element {
  return (
    <span>
      <span className="text-label">{label} </span>
      <span className="text-value">{value}</span>
    </span>
  );
}

function Properties(): React.JSX.Element | null {
  const level = useAppStore((s) => s.campaign.level);
  const klass = useAppStore((s) => s.campaign.class);
  const route = useAppStore((s) => s.route);
  if (level === 0 && !route) return null;
  return (
    <p className="flex justify-center gap-4 text-[13px]">
      {level > 0 && <Prop label={klass ? `${klass} level` : 'Level'} value={level} />}
      {route && <Prop label="Step" value={`${route.edge + 1} / ${route.edge_count}`} />}
    </p>
  );
}

// The route covers where to go; from the old guidance only the build's gem claims add anything.
function GemClaim(): React.JSX.Element | null {
  const guidance = useAppStore((s) => s.guidance);
  if (guidance?.kind !== 'quest_reward') return null;
  return <p className="text-[14px] leading-snug text-gem">{guidance.headline}</p>;
}

function NoLogHint(): React.JSX.Element | null {
  const mode = useAppStore((s) => s.source.mode);
  const error = useAppStore((s) => s.source.error);
  const holdKey = useAppStore((s) => s.hotkeys.menuHold);
  if (mode !== 'idle') return null;
  return (
    <p className="font-body text-[13px] leading-snug text-value">
      {error ?? 'Client.txt not found.'} Hold {holdLabel(holdKey)} in the game, open Settings
      and pick logs/Client.txt in the game folder.
    </p>
  );
}

function RouteNav(): React.JSX.Element {
  const keys = useAppStore((s) => s.hotkeys);
  return (
    <div className="flex items-center justify-center gap-1.5 pt-1">
      <button className="btn" onClick={() => void routePrev()}>
        Back
      </button>
      <span className="font-body text-[11px] text-label">
        {shortcutLabel(keys.routePrev)} / {shortcutLabel(keys.routeNext)}
      </span>
      <button className="btn" onClick={() => void routeNext()}>
        Next
      </button>
    </div>
  );
}

export function StatusPanel(): React.JSX.Element {
  const zone = useAppStore((s) => s.campaign.zone);
  const act = useAppStore((s) => s.campaign.act);
  const location = useAppStore((s) => s.campaign.location_kind);
  const grabbed = useAppStore((s) => s.ui.grabbed);
  const route = useAppStore((s) => s.route);
  const actions = hasActions(route);

  const tint = TINT;
  const subtitle = act != null ? `Act ${act}${location === 'town' ? ' town' : ''}` : undefined;

  return (
    <div className="tooltip">
      <TooltipHeader tint={tint} title={zone?.name ?? 'Waiting for the game'} subtitle={subtitle} />
      <div className="px-4 pt-1.5 pb-3">
        <Properties />
        <TooltipSep tint={tint} />
        <NoLogHint />
        {route && !actions ? <PassThrough /> : <HereSteps />}
        <GemClaim />
        {actions && (
          <>
            <TooltipSep tint={tint} />
            <ThenStep />
          </>
        )}
        {grabbed && <RouteNav />}
      </div>
    </div>
  );
}
