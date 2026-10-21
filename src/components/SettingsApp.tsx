import type { ReactNode } from 'react';

import {
  clearBuild,
  getSettings,
  importBuildFile,
  pasteBuild,
  pickLogFile,
  routeNext,
  routePrev,
  routeReset,
  saveSettings,
  setRouteEnabled,
  snapOverlayBottomCenter,
  startLive,
  startMove,
  startReplay,
  stopSource,
} from '../ipc/commands';
import type { Settings } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';
import { ReplayControls } from './ReplayControls';

// Read-modify-write: the backend saves the route position and anchor into the same file.
async function save(patch: Partial<Settings>): Promise<void> {
  const current = await getSettings();
  await saveSettings({ ...current, ...patch });
}

function Section({ title, children }: { title: string; children: ReactNode }): React.JSX.Element {
  return (
    <section className="flex flex-col gap-2.5">
      <h2 className="border-b border-frame-dark pb-1 font-caps text-[16px] text-currency">{title}</h2>
      {children}
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }): React.JSX.Element {
  return (
    <div className="flex min-h-7 items-center gap-3">
      <span className="w-32 shrink-0 font-caps text-[13px] text-label">{label}</span>
      <div className="flex flex-1 flex-wrap items-center gap-1.5">{children}</div>
    </div>
  );
}

function Toggle({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }): React.JSX.Element {
  return (
    <button className="btn" role="switch" aria-checked={on} aria-pressed={on} onClick={() => onChange(!on)}>
      {on ? 'On' : 'Off'}
    </button>
  );
}

function DisplaySection(): React.JSX.Element {
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
            void save({ textScale: v });
          }}
          className="flex-1"
        />
        <span className="w-11 text-right text-[13px] text-value">{Math.round(textScale * 100)}%</span>
      </Row>
      <Row label="Background">
        <input
          type="range"
          min={0.2}
          max={1}
          step={0.05}
          value={opacity}
          aria-label="Background opacity"
          onChange={(e) => {
            const v = Number(e.target.value);
            setOpacity(v);
            void save({ opacity: v });
          }}
          className="flex-1"
        />
        <span className="w-11 text-right text-[13px] text-value">{Math.round(opacity * 100)}%</span>
      </Row>
      <Row label="Position">
        <button className="btn" onClick={() => void snapOverlayBottomCenter()}>
          Bottom center
        </button>
        <button className="btn" onClick={() => void startMove()}>
          Move by hand
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
      <Row label="Position">
        <span className="text-[13px] text-value">
          {route ? `Step ${route.edge + 1} of ${route.edge_count}, ${route.section}` : 'No route'}
        </span>
      </Row>
      <Row label="Step">
        <button className="btn" onClick={() => void routePrev()}>
          Back
        </button>
        <button className="btn" onClick={() => void routeNext()}>
          Next
        </button>
        <button className="btn" onClick={() => void routeReset()}>
          Start over
        </button>
      </Row>
    </Section>
  );
}

function LogSection(): React.JSX.Element {
  const mode = useAppStore((s) => s.source.mode);
  const path = useAppStore((s) => s.source.path);
  return (
    <Section title="Game log">
      <Row label="Client.txt">
        <span className="min-w-0 flex-1 truncate font-body text-[13px] text-value" title={path ?? ''}>
          {mode === 'idle' ? 'Not found' : (path ?? '')}
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
          <button className="btn" onClick={() => void stopSource()}>
            Stop
          </button>
        )}
      </Row>
      <ReplayControls />
    </Section>
  );
}

function BuildSection(): React.JSX.Element {
  const build = useAppStore((s) => s.build);
  const setBuild = useAppStore((s) => s.setBuild);
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
          onClick={async () => {
            try {
              setBuild(await pasteBuild());
            } catch {
              /* nothing valid on the clipboard - keep the current build */
            }
          }}
        >
          Paste code
        </button>
        {build && (
          <button className="btn" onClick={() => void clearBuild()}>
            Clear
          </button>
        )}
      </Row>
    </Section>
  );
}

const HOTKEYS: [string, string][] = [
  ['Hold Esc', 'Open the quick menu'],
  ['Ctrl+Alt+Space', 'Move the overlay, press again to lock'],
  ['Ctrl+Alt+PageDown', 'Next route step'],
  ['Ctrl+Alt+PageUp', 'Previous route step'],
];

function HotkeySection(): React.JSX.Element {
  return (
    <Section title="Hotkeys">
      <dl className="grid grid-cols-[8rem_1fr] gap-x-3 gap-y-1.5 text-[13px]">
        {HOTKEYS.map(([key, what]) => (
          <div key={key} className="contents">
            <dt className="text-value">{key}</dt>
            <dd className="text-label">{what}</dd>
          </div>
        ))}
      </dl>
    </Section>
  );
}

export function SettingsApp(): React.JSX.Element {
  return (
    <main className="tooltip mx-auto flex max-w-[34rem] flex-col gap-6 px-6 py-5 text-left">
      <DisplaySection />
      <RouteSection />
      <LogSection />
      <BuildSection />
      <HotkeySection />
    </main>
  );
}
