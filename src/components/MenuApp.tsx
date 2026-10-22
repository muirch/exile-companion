import { useEffect, useRef } from 'react';

import {
  closeMenu,
  openSettings,
  quitApp,
  setRouteEnabled,
  startMove,
  updateSettings,
} from '../ipc/commands';
import { isTauri } from '../ipc/env';
import { onMenuOpened } from '../ipc/events';
import { useAppStore } from '../store/useAppStore';
import { shortcutLabel } from './HotkeySection';
import { Brand, TooltipHeader, TooltipSep } from './Tooltip';

const TINT = 'var(--color-currency)';

function Row({
  label,
  hint,
  onClick,
  state,
  first,
}: {
  label: string;
  hint?: string;
  onClick: () => void;
  state?: boolean;
  first?: boolean;
}): React.JSX.Element {
  return (
    <button
      data-first={first || undefined}
      onClick={onClick}
      role={state === undefined ? 'menuitem' : 'menuitemcheckbox'}
      aria-checked={state}
      className="flex w-full items-baseline justify-between gap-3 px-3 py-1.5 text-left text-[14px] text-value hover:bg-white/6 focus-visible:bg-white/6 focus-visible:outline-none"
    >
      <span>{label}</span>
      {state !== undefined && (
        <span className={state ? 'text-quest' : 'text-label'}>{state ? 'On' : 'Off'}</span>
      )}
      {hint && <span className="font-body text-[11px] text-label">{hint}</span>}
    </button>
  );
}

export function MenuApp(): React.JSX.Element {
  const routeEnabled = useAppStore((s) => s.routeEnabled);
  const compact = useAppStore((s) => s.ui.compact);
  const moveKey = useAppStore((s) => s.hotkeys.moveOverlay);
  const panel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const focusFirst = (): void => {
      panel.current?.querySelector<HTMLButtonElement>('[data-first]')?.focus();
    };
    focusFirst();
    if (!isTauri()) return;
    const off = onMenuOpened(focusFirst);
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') void closeMenu();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      void off.then((f) => f());
      window.removeEventListener('keydown', onKey);
    };
  }, []);

  return (
    <div ref={panel} role="menu" className="tooltip m-1 w-[228px] bg-black/90 pb-1.5">
      <TooltipHeader tint={TINT} title={<Brand />} />
      <div className="h-1.5" />
      <Row
        first
        label="Leveling route"
        state={routeEnabled}
        onClick={() => void setRouteEnabled(!routeEnabled)}
      />
      <Row
        label="Compact"
        state={compact}
        onClick={() => void updateSettings({ compact: !compact })}
      />
      <TooltipSep tint={TINT} />
      <Row label="Settings" onClick={() => void openSettings()} />
      <Row
        label="Move overlay"
        hint={shortcutLabel(moveKey)}
        onClick={() => void startMove()}
      />
      <TooltipSep tint={TINT} />
      <Row label="Quit poebo" onClick={() => void quitApp()} />
    </div>
  );
}
