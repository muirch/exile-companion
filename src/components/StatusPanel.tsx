import { useEffect, useState } from 'react';

import { routeNext, routePrev } from '../ipc/commands';
import { useAppStore } from '../store/useAppStore';
import { holdLabel, shortcutLabel } from './HotkeySection';
import { ActDone, HereSteps, PassThrough, ThenStep, hasActions } from './RoutePanel';
import { TooltipHeader, TooltipSep } from './Tooltip';

// Sand (the currency item's tint) for every header; towns are told apart by the subtitle.
const TINT = 'var(--color-currency)';

// Act, level and step share the band's subtitle, so the first line under the band is an action.
function useSubtitle(): string | undefined {
  const act = useAppStore((s) => s.campaign.act);
  const location = useAppStore((s) => s.campaign.location_kind);
  const level = useAppStore((s) => s.campaign.level);
  const klass = useAppStore((s) => s.campaign.class);
  const route = useAppStore((s) => s.route);
  const parts = [
    act != null ? `Act ${act}${location === 'town' ? ' town' : ''}` : null,
    level > 0 ? (klass ? `${klass} level ${level}` : `Level ${level}`) : null,
    route ? `Step ${route.section_edge + 1} / ${route.section_edge_count}` : null,
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(' · ') : undefined;
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
const PEEK_HINTS_MS = 6000;

// A tap of the menu key shows the step's hints for a few seconds; compact has no room for them
// otherwise.
function usePeekHints(): boolean {
  const peeks = useAppStore((s) => s.ui.peeks);
  const [state, setState] = useState({ peeks, on: false });
  if (state.peeks !== peeks) setState({ peeks, on: true });
  useEffect(() => {
    if (!state.on) return;
    const t = setTimeout(() => setState((s) => ({ ...s, on: false })), PEEK_HINTS_MS);
    return () => clearTimeout(t);
  }, [state]);
  return state.on;
}

export function CompactPanel(): React.JSX.Element {
  const hints = usePeekHints();
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
      <ActDone />
      <OffRouteNote />
      <NoLogHint />
      {route && !actions ? <PassThrough /> : <HereSteps hints={hints} />}
      {actions && <ThenStep />}
    </div>
  );
}

export function StatusPanel(): React.JSX.Element {
  const zone = useAppStore((s) => s.campaign.zone);
  const route = useAppStore((s) => s.route);
  const actions = hasActions(route);
  const subtitle = useSubtitle();
  const tint = TINT;

  return (
    <div className="tooltip">
      <TooltipHeader tint={tint} title={zone?.name ?? 'Waiting for the game'} subtitle={subtitle} />
      <div className="px-4 pt-2 pb-3">
        <ActDone />
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
