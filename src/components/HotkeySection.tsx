import { useCallback, useEffect, useState } from 'react';

import { getHotkeys, pauseHotkeys, resumeHotkeys, setHotkeys } from '../ipc/commands';
import type { BindError, HotkeySettings, Hotkeys } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';

type Bindable = Exclude<keyof Hotkeys, 'menuHold'>;

const ROWS: { key: Bindable; label: string }[] = [
  { key: 'moveOverlay', label: 'Move overlay' },
  { key: 'routePrev', label: 'Previous step' },
  { key: 'routeNext', label: 'Next step' },
];

const HOLD_KEYS: { value: string; label: string }[] = [
  { value: 'Backquote', label: 'Tilde' },
  { value: 'Tab', label: 'Tab' },
];

const KEY_NAMES: Record<string, string> = {
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backslash: '\\',
};

export function holdLabel(key: string): string {
  return HOLD_KEYS.find((k) => k.value === key)?.label ?? key;
}

export function shortcutLabel(shortcut: string): string {
  if (!shortcut) return 'Not set';
  return shortcut
    .split('+')
    .map((t) => KEY_NAMES[t] ?? t.replace(/^Key/, '').replace(/^Digit/, '').replace(/^Numpad/, 'Num '))
    .join('+');
}

const MODIFIERS = ['Control', 'Alt', 'Shift', 'Meta'];

// global-hotkey syntax: modifiers, then the W3C key code.
function fromEvent(e: KeyboardEvent): string | null {
  if (MODIFIERS.some((m) => e.key === m)) return null;
  const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift'].filter(Boolean);
  return [...mods, e.code].join('+');
}

function Binding({
  value,
  recording,
  onRecord,
}: {
  value: string;
  recording: boolean;
  onRecord: () => void;
}): React.JSX.Element {
  return (
    <button className="btn min-w-28 font-body" aria-pressed={recording} onClick={onRecord}>
      {recording ? 'Press keys…' : shortcutLabel(value)}
    </button>
  );
}

export function HotkeySection(): React.JSX.Element {
  const [cfg, setCfg] = useState<HotkeySettings | null>(null);
  const [recording, setRecording] = useState<Bindable | null>(null);
  const [errors, setErrors] = useState<BindError[]>([]);

  useEffect(() => {
    void getHotkeys().then(setCfg);
  }, []);

  const commit = useCallback(async (next: HotkeySettings): Promise<void> => {
    setCfg(next);
    useAppStore.getState().setHotkeys(next.hotkeys);
    setErrors(await setHotkeys(next));
  }, []);

  useEffect(() => {
    if (!recording || !cfg) return;
    void pauseHotkeys();
    const onKey = (e: KeyboardEvent): void => {
      e.preventDefault();
      if (e.key === 'Escape') {
        setRecording(null);
        void resumeHotkeys().then(setErrors);
        return;
      }
      const shortcut = e.key === 'Backspace' ? '' : fromEvent(e);
      if (shortcut === null) return;
      setRecording(null);
      void commit({ ...cfg, hotkeys: { ...cfg.hotkeys, [recording]: shortcut } });
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [recording, cfg, commit]);

  if (!cfg) return <p className="text-[13px] text-label">Loading hotkeys…</p>;
  const errorFor = (key: string): BindError | undefined => errors.find((e) => e.action === key);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex min-h-7 items-center gap-3">
        <span className="w-32 shrink-0 font-caps text-[13px] text-label">Quick menu</span>
        <span className="font-body text-[13px] text-plain">Hold</span>
        <select
          className="btn font-body"
          value={cfg.hotkeys.menuHold}
          aria-label="Key to hold for the quick menu"
          onChange={(e) =>
            void commit({ ...cfg, hotkeys: { ...cfg.hotkeys, menuHold: e.target.value } })
          }
        >
          {HOLD_KEYS.map((k) => (
            <option key={k.value} value={k.value}>
              {k.label}
            </option>
          ))}
        </select>
      </div>
      {ROWS.map((row) => (
        <div key={row.key} className="flex min-h-7 items-center gap-3">
          <span className="w-32 shrink-0 font-caps text-[13px] text-label">{row.label}</span>
          <Binding
            value={cfg.hotkeys[row.key]}
            recording={recording === row.key}
            onRecord={() => setRecording(recording === row.key ? null : row.key)}
          />
          {errorFor(row.key) && (
            <span className="font-body text-[12px] text-corrupted">Taken or invalid</span>
          )}
        </div>
      ))}

      <div className="mt-2 flex min-h-7 items-center gap-3">
        <span className="w-32 shrink-0 font-caps text-[13px] text-label">Relog</span>
        <button
          className="btn"
          role="switch"
          aria-checked={cfg.relogEnabled}
          onClick={() => void commit({ ...cfg, relogEnabled: !cfg.relogEnabled })}
        >
          {cfg.relogEnabled ? 'On' : 'Off'}
        </button>
        <Binding
          value={cfg.hotkeys.relog}
          recording={recording === 'relog'}
          onRecord={() => setRecording(recording === 'relog' ? null : 'relog')}
        />
        {errorFor('relog') && (
          <span className="font-body text-[12px] text-corrupted">Taken or invalid</span>
        )}
      </div>
      <p className="ml-35 font-body text-[12px] leading-snug text-label italic">
        Types /exit in the game chat to go back to character select, like Awakened PoE Trade's F9.
        This is the only thing poebo sends to the game: one command per key press, only while the
        game is in front.
      </p>
      <p className="font-body text-[12px] text-label italic">
        Click a key to change it. Backspace clears it, Esc cancels.
      </p>
    </div>
  );
}
