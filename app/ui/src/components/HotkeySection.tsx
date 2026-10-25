import { useCallback, useEffect, useState } from 'react';

import { getHotkeys, pauseHotkeys, resumeHotkeys, setHotkeys } from '../ipc/commands';
import type { BindError, HotkeySettings, Hotkeys } from '../ipc/types';
import { Button, Note, Row, Select, Toggle } from '../kit';
import { useAppStore } from '../store/useAppStore';

type Bindable = Exclude<keyof Hotkeys, 'menuHold'>;

const ROWS: { key: Bindable; label: string }[] = [
  { key: 'moveOverlay', label: 'Move overlay' },
  { key: 'routePrev', label: 'Previous step' },
  { key: 'routeNext', label: 'Next step' },
  { key: 'openChat', label: 'Assistant' },
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
  dim = false,
  onRecord,
}: {
  value: string;
  recording: boolean;
  /** Still rebindable, but the key does nothing while its feature is off. */
  dim?: boolean;
  onRecord: () => void;
}): React.JSX.Element {
  return (
    <Button
      literal
      className={dim && !recording ? 'min-w-28 opacity-50' : 'min-w-28'}
      aria-pressed={recording}
      onClick={onRecord}
    >
      {recording ? 'Press keys…' : shortcutLabel(value)}
    </Button>
  );
}

// global-hotkey says "HotKey already registered" when another program holds the shortcut.
export function bindErrorText(error: BindError): string {
  return /already registered/i.test(error.error) ? 'Another program uses this' : "Can't bind this key";
}

function BindNote({ error }: { error: BindError | undefined }): React.JSX.Element | null {
  if (!error) return null;
  return <span className="font-body text-[12px] text-corrupted">{bindErrorText(error)}</span>;
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
      <Row label="Quick menu">
        <span className="font-body text-[13px] text-plain">Hold</span>
        <Select
          value={cfg.hotkeys.menuHold}
          options={HOLD_KEYS}
          label="Key to hold for the quick menu"
          onChange={(v) => void commit({ ...cfg, hotkeys: { ...cfg.hotkeys, menuHold: v } })}
        />
      </Row>
      {ROWS.map((row) => (
        <Row key={row.key} label={row.label}>
          <Binding
            value={cfg.hotkeys[row.key]}
            recording={recording === row.key}
            onRecord={() => setRecording(recording === row.key ? null : row.key)}
          />
          <BindNote error={errorFor(row.key)} />
        </Row>
      ))}

      <Row label="Relog" className="mt-2">
        <Toggle
          on={cfg.relogEnabled}
          label="Relog"
          onChange={(v) => void commit({ ...cfg, relogEnabled: v })}
        />
        <Binding
          value={cfg.hotkeys.relog}
          recording={recording === 'relog'}
          dim={!cfg.relogEnabled}
          onRecord={() => setRecording(recording === 'relog' ? null : 'relog')}
        />
        <BindNote error={errorFor('relog')} />
      </Row>
      <Note>
        Types /exit in the game chat to go back to character select, like Awakened PoE Trade's F9.
      </Note>

      <Row label="Hideout" className="mt-2">
        <Toggle
          on={cfg.hideoutEnabled}
          label="Hideout"
          onChange={(v) => void commit({ ...cfg, hideoutEnabled: v })}
        />
        <Binding
          value={cfg.hotkeys.hideout}
          recording={recording === 'hideout'}
          dim={!cfg.hideoutEnabled}
          onRecord={() => setRecording(recording === 'hideout' ? null : 'hideout')}
        />
        <BindNote error={errorFor('hideout')} />
      </Row>
      <Note>Types /hideout in the game chat, like Awakened PoE Trade's F5.</Note>

      <Row label="Item check" className="mt-2">
        <Toggle
          on={cfg.itemCheckEnabled}
          label="Item check"
          onChange={(v) => void commit({ ...cfg, itemCheckEnabled: v })}
        />
        <Binding
          value={cfg.hotkeys.itemCheck}
          recording={recording === 'itemCheck'}
          dim={!cfg.itemCheckEnabled}
          onRecord={() => setRecording(recording === 'itemCheck' ? null : 'itemCheck')}
        />
        <BindNote error={errorFor('itemCheck')} />
      </Row>
      <Note>
        Point at an item in the game and press it. The overlay says what the item would change for
        your build, measured with Path of Building, or where your build uses a gem. It copies the
        item with Ctrl+C, like Awakened PoE Trade's Ctrl+D.
      </Note>

      <Row label="Gem search" className="mt-2">
        <Toggle
          on={cfg.vendorSearchEnabled}
          label="Gem search"
          onChange={(v) => void commit({ ...cfg, vendorSearchEnabled: v })}
        />
        <Binding
          value={cfg.hotkeys.vendorSearch}
          recording={recording === 'vendorSearch'}
          dim={!cfg.vendorSearchEnabled}
          onRecord={() => setRecording(recording === 'vendorSearch' ? null : 'vendorSearch')}
        />
        <BindNote error={errorFor('vendorSearch')} />
      </Row>
      <Note>
        With a vendor window open, types a search for the gems the route says to buy here, and the
        game highlights them. If they don't fit in one search, press again for the rest.
      </Note>
      <Note>
        Relog, hideout, item check and gem search are the only keys that send anything to the game:
        one action per key press, and only while the game is in front.
      </Note>
      <Note flush>Click a key to change it. Backspace clears it, Esc cancels.</Note>
    </div>
  );
}
