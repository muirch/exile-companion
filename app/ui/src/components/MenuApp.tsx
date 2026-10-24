import { useEffect, useRef, useState } from 'react';

import {
  closeMenu,
  openChat,
  openSettings,
  quitApp,
  routeNext,
  routePrev,
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
  label: React.ReactNode;
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

// The menu opens under the cursor in the middle of play, so quitting takes a second click.
function QuitRow(): React.JSX.Element {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(t);
  }, [armed]);
  return (
    <Row
      label={
        armed ? (
          'Click again to quit'
        ) : (
          <>
            Quit <Brand />
          </>
        )
      }
      onClick={() => (armed ? void quitApp() : setArmed(true))}
    />
  );
}

// Up and Down walk the rows, as the menu role promises.
function moveFocus(panel: HTMLElement, step: number): void {
  const rows = [...panel.querySelectorAll<HTMLButtonElement>('button')];
  const at = rows.indexOf(document.activeElement as HTMLButtonElement);
  rows[(at + step + rows.length) % rows.length]?.focus();
}

export function MenuApp(): React.JSX.Element {
  const routeEnabled = useAppStore((s) => s.routeEnabled);
  const compact = useAppStore((s) => s.ui.compact);
  const keys = useAppStore((s) => s.hotkeys);
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
    <div
      ref={panel}
      role="menu"
      tabIndex={-1}
      className="tooltip m-1 w-[228px] bg-black/90 pb-1.5 outline-none"
      onKeyDown={(e) => {
        if (!panel.current) return;
        if (e.key === 'ArrowDown') moveFocus(panel.current, 1);
        if (e.key === 'ArrowUp') moveFocus(panel.current, -1);
      }}
    >
      <TooltipHeader tint={TINT} title={<Brand />} />
      <div className="h-1.5" />
      <Row
        first
        label="Show route"
        state={routeEnabled}
        onClick={() => void setRouteEnabled(!routeEnabled)}
      />
      <Row
        label="Full layout"
        state={!compact}
        onClick={() => void updateSettings({ compact: !compact })}
      />
      <TooltipSep tint={TINT} />
      {/* The menu stays open so a wrong step can be fixed with a few clicks. */}
      <Row
        label="Previous step"
        hint={shortcutLabel(keys.routePrev)}
        onClick={() => void routePrev()}
      />
      <Row label="Next step" hint={shortcutLabel(keys.routeNext)} onClick={() => void routeNext()} />
      <TooltipSep tint={TINT} />
      <Row label="Ask the assistant" hint={shortcutLabel(keys.openChat)} onClick={() => void openChat()} />
      <Row label="Settings" onClick={() => void openSettings()} />
      <Row
        label="Move overlay"
        hint={shortcutLabel(keys.moveOverlay)}
        onClick={() => void startMove()}
      />
      <TooltipSep tint={TINT} />
      <QuitRow />
    </div>
  );
}
