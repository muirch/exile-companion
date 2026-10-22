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
      <span className="text-plain">{label} </span>
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
      {route && (
        <Prop label="Step" value={`${route.section_edge + 1} / ${route.section_edge_count}`} />
      )}
    </p>
  );
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

export function RouteNav(): React.JSX.Element {
  const keys = useAppStore((s) => s.hotkeys);
  return (
    <div className="flex items-center justify-center gap-1.5 pt-1">
      <button className="btn" onClick={() => void routePrev()}>
        Back
      </button>
      <span className="font-body text-[11px] text-plain">
        {shortcutLabel(keys.routePrev)} / {shortcutLabel(keys.routeNext)}
      </span>
      <button className="btn" onClick={() => void routeNext()}>
        Next
      </button>
    </div>
  );
}

// The tracker holds its place in hideouts, maps and areas off the route, so the steps shown are
// for somewhere else.
function useOffRoute(): boolean {
  const zone = useAppStore((s) => s.campaign.zone);
  const route = useAppStore((s) => s.route);
  return !!zone && !!route && zone.id !== route.area_id;
}

function OffRouteNote(): React.JSX.Element | null {
  if (!useOffRoute()) return null;
  return <p className="text-[13px] text-plain">Off route, showing your last step</p>;
}

// No header band, properties line or hints: where you are in one grey line, then the route.
export function CompactPanel(): React.JSX.Element {
  const zone = useAppStore((s) => s.campaign.zone);
  const act = useAppStore((s) => s.campaign.act);
  const route = useAppStore((s) => s.route);
  const actions = hasActions(route);
  const where = [
    zone?.name ?? 'Waiting for the game',
    act != null ? `Act ${act}` : null,
    route ? `${route.section_edge + 1} / ${route.section_edge_count}` : null,
  ]
    .filter(Boolean)
    .join(' · ');

  return (
    <div className="tooltip flex flex-col gap-1 px-3 pt-1.5 pb-2">
      <p className="truncate text-[13px] text-plain">{where}</p>
      <OffRouteNote />
      <NoLogHint />
      {route && !actions ? <PassThrough /> : <HereSteps hints={false} />}
      {actions && <ThenStep />}
    </div>
  );
}

export function StatusPanel(): React.JSX.Element {
  const zone = useAppStore((s) => s.campaign.zone);
  const act = useAppStore((s) => s.campaign.act);
  const location = useAppStore((s) => s.campaign.location_kind);
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
        <OffRouteNote />
        <NoLogHint />
        {route && !actions ? <PassThrough /> : <HereSteps />}
        {actions && (
          <>
            <TooltipSep tint={tint} />
            <ThenStep />
          </>
        )}
      </div>
    </div>
  );
}
