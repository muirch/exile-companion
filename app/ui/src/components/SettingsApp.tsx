import { useEffect, useRef, useState, type ReactNode } from 'react';

import {
  clearBuild,
  closeSettings,
  getSettings,
  importBuildFile,
  pasteBuild,
  pickLogFile,
  routeNext,
  routePrev,
  routeReset,
  setRouteEnabled,
  snapOverlayBottomCenter,
  startLive,
  startMove,
  startReplay,
  stopSource,
  updateSettings,
} from '../ipc/commands';
import type { GemHints, PresetStage } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';
import { HotkeySection } from './HotkeySection';
import { ReplayControls } from './ReplayControls';
import { Brand, TooltipHeader } from './Tooltip';

// Each group is its own item tooltip: a header band over a see-through black body.
function Section({ title, children }: { title: string; children: ReactNode }): React.JSX.Element {
  return (
    <section className="tooltip bg-black/80">
      <TooltipHeader tint="var(--color-currency)" title={title} />
      <div className="flex flex-col gap-2 px-5 pt-3 pb-4 text-left">{children}</div>
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }): React.JSX.Element {
  return (
    <div className="flex min-h-7 items-center gap-3">
      <span className="w-32 shrink-0 font-caps text-[13px] text-label">{label}</span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">{children}</div>
    </div>
  );
}

// Drops progress, the build or the game log: the first click arms it, the second does it.
// A second click right after the first is a double click or a bouncing mouse button, not a
// confirmation.
const CONFIRM_AFTER_MS = 500;

function ConfirmButton({
  label,
  confirm,
  onConfirm,
}: {
  label: string;
  confirm: string;
  onConfirm: () => void;
}): React.JSX.Element {
  const [armed, setArmed] = useState(false);
  const armedAt = useRef(0);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(t);
  }, [armed]);
  return (
    <button
      className="btn"
      aria-pressed={armed}
      onClick={() => {
        if (!armed) {
          armedAt.current = Date.now();
          setArmed(true);
        } else if (Date.now() - armedAt.current >= CONFIRM_AFTER_MS) {
          setArmed(false);
          onConfirm();
        }
      }}
    >
      {armed ? confirm : label}
    </button>
  );
}

function Toggle({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }): React.JSX.Element {
  return (
    <button className="btn" role="switch" aria-checked={on} onClick={() => onChange(!on)}>
      {on ? 'On' : 'Off'}
    </button>
  );
}

function Choice<T extends string | number | boolean>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string; literal?: boolean }[];
  onChange: (v: T) => void;
}): React.JSX.Element {
  return (
    <>
      {options.map((o) => (
        <button
          key={String(o.value)}
          className={o.literal ? 'btn font-body' : 'btn'}
          aria-pressed={value === o.value}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </>
  );
}

const LAYOUTS = [
  { value: false, label: 'Full' },
  { value: true, label: 'Compact' },
];

const FADES = [
  { value: 0, label: 'Off' },
  { value: 5, label: '5 s', literal: true },
  { value: 10, label: '10 s', literal: true },
  { value: 30, label: '30 s', literal: true },
];

function DisplaySection(): React.JSX.Element {
  const compact = useAppStore((s) => s.ui.compact);
  const fadeAfter = useAppStore((s) => s.ui.fadeAfter);
  const opacity = useAppStore((s) => s.ui.opacity);
  const textScale = useAppStore((s) => s.ui.textScale);
  const setOpacity = useAppStore((s) => s.setOpacity);
  const setTextScale = useAppStore((s) => s.setTextScale);
  return (
    <Section title="Overlay">
      <Row label="Text size">
        <input
          type="range"
          min={0.8}
          max={1.5}
          step={0.05}
          value={textScale}
          aria-label="Text size"
          onChange={(e) => {
            const v = Number(e.target.value);
            setTextScale(v);
            void updateSettings({ textScale: v });
          }}
          className="flex-1"
        />
        <span className="w-11 text-right text-[13px] text-value">{Math.round(textScale * 100)}%</span>
      </Row>
      <Row label="Background">
        <input
          type="range"
          min={0.7}
          max={1}
          step={0.05}
          value={opacity}
          aria-label="Background opacity"
          onChange={(e) => {
            const v = Number(e.target.value);
            setOpacity(v);
            void updateSettings({ opacity: v });
          }}
          className="flex-1"
        />
        <span className="w-11 text-right text-[13px] text-value">{Math.round(opacity * 100)}%</span>
      </Row>
      <Row label="Layout">
        <Choice value={compact} options={LAYOUTS} onChange={(v) => void updateSettings({ compact: v })} />
      </Row>
      <Row label="Fade out">
        <Choice
          value={fadeAfter}
          options={FADES}
          onChange={(v) => void updateSettings({ fadeAfter: v })}
        />
      </Row>
      <p className="ml-35 font-body text-[12px] leading-snug text-label italic">
        Hides the overlay after that long without a new step. It comes back with the next area or
        step, or when you tap the menu key.
      </p>
      <Row label="Position">
        <button className="btn" onClick={() => void snapOverlayBottomCenter()}>
          Bottom center
        </button>
        <button className="btn" onClick={() => void startMove()}>
          Move overlay
        </button>
      </Row>
    </Section>
  );
}

function RouteSection(): React.JSX.Element {
  const enabled = useAppStore((s) => s.routeEnabled);
  const route = useAppStore((s) => s.route);
  return (
    <Section title="Leveling route">
      <Row label="Show route">
        <Toggle on={enabled} onChange={(v) => void setRouteEnabled(v)} />
      </Row>
      <Row label="Progress">
        <span className="text-[13px] text-value">
          {route
            ? `${route.section}, step ${route.section_edge + 1} of ${route.section_edge_count}`
            : 'No route'}
        </span>
      </Row>
      <Row label="Step">
        <button className="btn" onClick={() => void routePrev()}>
          Back
        </button>
        <button className="btn" onClick={() => void routeNext()}>
          Next
        </button>
        <ConfirmButton
          label="Start over"
          confirm="Click again to start over"
          onConfirm={() => void routeReset()}
        />
      </Row>
      <p className="ml-35 font-body text-[12px] leading-snug text-label italic">
        The route is exile-leveling's (MIT), the speedrun route by HeartofPhos and contributors.
      </p>
    </Section>
  );
}

function LogSection(): React.JSX.Element {
  const mode = useAppStore((s) => s.source.mode);
  const path = useAppStore((s) => s.source.path);
  const [testing, setTesting] = useState(false);
  const showTesting = testing || mode === 'replay';
  return (
    <Section title="Game log">
      <Row label="Client.txt">
        {/* Cut from the left, so the end of the path, logs\Client.txt, stays in view. */}
        <span
          dir="rtl"
          className="min-w-0 flex-1 truncate text-left font-body text-[13px] text-value"
          title={path ?? ''}
        >
          <bdi>{mode === 'idle' ? 'Not found' : (path ?? '')}</bdi>
        </span>
      </Row>
      <Row label="">
        <button
          className="btn"
          onClick={async () => {
            const picked = await pickLogFile();
            if (picked) await startLive(picked);
          }}
        >
          Pick Client.txt
        </button>
      </Row>
      <Row label="Testing">
        {mode !== 'replay' && (
          <button className="btn" aria-expanded={showTesting} onClick={() => setTesting(!testing)}>
            {showTesting ? 'Hide' : 'Show'}
          </button>
        )}
        {showTesting && (
          <>
            <button
              className="btn"
              onClick={async () => {
                const picked = await pickLogFile();
                if (picked) await startReplay(picked, 'X5');
              }}
            >
              Replay a log
            </button>
            {mode !== 'idle' && (
              <ConfirmButton
                label="Stop"
                confirm="Click again to stop"
                onConfirm={() => void stopSource()}
              />
            )}
          </>
        )}
      </Row>
      {showTesting && (
        <p className="ml-35 font-body text-[12px] leading-snug text-label italic">
          Plays a saved log back to check the route without the game. Stop leaves Exile Companion without a
          log until you pick one again.
        </p>
      )}
      {mode === 'replay' && (
        <Row label="Replay">
          <div className="flex-1">
            <ReplayControls />
          </div>
        </Row>
      )}
    </Section>
  );
}

const GEM_HINTS: { value: GemHints; label: string }[] = [
  { value: 'off', label: 'Off' },
  { value: 'rewards', label: 'Rewards' },
  { value: 'rewardsAndVendors', label: 'Rewards and buys' },
];

function GemHintsRow(): React.JSX.Element {
  const [hints, setHints] = useState<GemHints>('rewardsAndVendors');
  useEffect(() => {
    void getSettings().then((s) => {
      if (s?.gemHints) setHints(s.gemHints);
    });
  }, []);
  return (
    <Row label="Gems in route">
      {GEM_HINTS.map((h) => (
        <button
          key={h.value}
          className="btn"
          aria-pressed={hints === h.value}
          onClick={() => {
            setHints(h.value);
            void updateSettings({ gemHints: h.value });
          }}
        >
          {h.label}
        </button>
      ))}
    </Row>
  );
}

const ACTS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

function stageValue(stage: PresetStage | undefined): string {
  if (stage === undefined || stage === 'auto') return 'auto';
  if (typeof stage === 'object') return `act${stage.act}`;
  return stage;
}

function stageFrom(value: string): PresetStage {
  if (value.startsWith('act')) return { act: Number(value.slice(3)) };
  return value as PresetStage;
}

// Separator presets are often "----- Mapping -----"; the dashes are only there for PoB's list.
export function presetName(title: string | null, i: number): string {
  const name = (title ?? '').replace(/^[\s\-=_*~#]+|[\s\-=_*~#]+$/g, '');
  return name || `Preset ${i + 1}`;
}

function autoLabel(level: number | null): string {
  if (level === null) return 'Auto, by gem level';
  if (level >= 100) return 'Auto, endgame';
  return `Auto, from level ${level}`;
}

// Gems are planned from the first skill set that has them. This lets the player say which act a
// set is for when its name doesn't, or leave a set out.
function PresetRows(): React.JSX.Element | null {
  const presets = useAppStore((s) => s.build?.presets);
  const [stages, setStages] = useState<PresetStage[]>([]);
  // A new build comes with its stages reset, so read them again.
  useEffect(() => {
    if (!presets) return;
    void getSettings().then((s) => setStages(s?.presetStages ?? []));
  }, [presets]);
  if (!presets || presets.length === 0) return null;

  const change = (i: number, stage: PresetStage): void => {
    const next = presets.map((_, j) => (j === i ? stage : (stages[j] ?? 'auto')));
    setStages(next);
    void updateSettings({ presetStages: next });
  };

  return (
    <>
      {presets.map((p, i) => (
        <Row key={i} label={i === 0 ? 'Presets' : ''}>
          <span
            className={
              p.gem_count === 0
                ? 'min-w-0 flex-1 truncate font-body text-[13px] text-label italic'
                : 'min-w-0 flex-1 truncate font-body text-[13px] text-value'
            }
            title={p.title ?? ''}
          >
            {presetName(p.title, i)}
          </span>
          {p.gem_count === 0 ? (
            <span className="font-body text-[12px] text-label italic">No gems</span>
          ) : (
            <select
              className="btn w-40 font-body"
              aria-label={`Stage of ${p.title || `preset ${i + 1}`}`}
              value={stageValue(stages[i])}
              onChange={(e) => change(i, stageFrom(e.target.value))}
            >
              <option value="auto">{autoLabel(p.auto_level)}</option>
              {ACTS.map((a) => (
                <option key={a} value={`act${a}`}>
                  From act {a}
                </option>
              ))}
              <option value="endgame">Endgame</option>
              <option value="skip">Leave out</option>
            </select>
          )}
        </Row>
      ))}
      <p className="ml-35 font-body text-[12px] leading-snug text-label italic">
        A gem is planned from the first preset it shows up in. Auto reads the act or level from the
        preset name and keeps the order. Name presets like Act 3 or lvl 28 and this is rarely needed.
      </p>
    </>
  );
}

function BuildSection(): React.JSX.Element {
  const build = useAppStore((s) => s.build);
  const setBuild = useAppStore((s) => s.setBuild);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const summary = build
    ? [
        [build.class_name, build.ascend_class_name].filter(Boolean).join(' '),
        build.active_spec_title,
        build.main_skill,
      ]
        .filter(Boolean)
        .join(', ')
    : 'None';
  return (
    <Section title="Path of Building">
      <Row label="Build">
        <span className="min-w-0 flex-1 truncate text-[13px] text-value">{summary}</span>
      </Row>
      <Row label="">
        <button
          className="btn"
          onClick={async () => {
            const s = await importBuildFile();
            if (s) setBuild(s);
          }}
        >
          Import file
        </button>
        <button
          className="btn"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            setError(null);
            try {
              setBuild(await pasteBuild());
            } catch (e) {
              setError(String(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? 'Loading…' : 'Paste code or link'}
        </button>
        {build && (
          <ConfirmButton
            label="Clear"
            confirm="Click again to clear"
            onConfirm={() => void clearBuild()}
          />
        )}
      </Row>
      {error && (
        <Row label="">
          <span role="alert" className="min-w-0 flex-1 font-body text-[12px] text-corrupted">
            {error}
          </span>
        </Row>
      )}
      <GemHintsRow />
      {build && build.not_in_campaign.length > 0 && (
        <Row label="Not in campaign">
          <span className="min-w-0 flex-1 font-body text-[13px] text-value">
            {build.not_in_campaign.join(', ')}
          </span>
        </Row>
      )}
      <PresetRows />
    </Section>
  );
}

function HotkeysGroup(): React.JSX.Element {
  return (
    <Section title="Hotkeys">
      <HotkeySection />
    </Section>
  );
}

// The window has no system frame. This bar moves it, like the overlay's move bar.
function TitleBar(): React.JSX.Element {
  return (
    <header
      data-tauri-drag-region
      className="flex h-8 shrink-0 cursor-move items-center gap-2 border-b border-edge bg-black pr-1 pl-3"
    >
      <span data-tauri-drag-region className="font-caps text-[13px] text-normal">
        <Brand />
      </span>
      <span data-tauri-drag-region className="font-caps text-[13px] text-label">
        Settings
      </span>
      <button
        className="btn ml-auto grid size-6 cursor-default place-items-center p-0"
        aria-label="Close settings"
        onClick={() => void closeSettings()}
      >
        <svg viewBox="0 0 12 12" className="size-3" aria-hidden="true">
          <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" fill="none" stroke="currentColor" strokeWidth="1.4" />
        </svg>
      </button>
    </header>
  );
}

export function SettingsApp(): React.JSX.Element {
  return (
    <div className="flex h-full flex-col">
      <TitleBar />
      <div className="min-h-0 flex-1 overflow-y-auto">
        <main className="mx-auto flex max-w-[34rem] flex-col gap-4 px-5 py-5">
          <DisplaySection />
          <RouteSection />
          <LogSection />
          <BuildSection />
          <HotkeysGroup />
        </main>
      </div>
    </div>
  );
}
