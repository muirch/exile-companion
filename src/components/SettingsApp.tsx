import type { ReactNode } from 'react';

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
import { HotkeySection } from './HotkeySection';
import { ReplayControls } from './ReplayControls';
import { TooltipHeader } from './Tooltip';

// Read-modify-write: the backend saves the route position and anchor into the same file.
async function save(patch: Partial<Settings>): Promise<void> {
  const current = await getSettings();
  await saveSettings({ ...current, ...patch });
}

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
      <div className="flex flex-1 flex-wrap items-center gap-1.5">{children}</div>
    </div>
  );
}

function Toggle({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }): React.JSX.Element {
  return (
    <button className="btn" role="switch" aria-checked={on} onClick={() => onChange(!on)}>
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
          min={0.5}
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
      <Row label="Progress">
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

function HotkeysGroup(): React.JSX.Element {
  return (
    <Section title="Hotkeys">
      <HotkeySection />
    </Section>
  );
}

// The window has no system frame; this bar moves it, like the overlay's move bar.
function TitleBar(): React.JSX.Element {
  return (
    <header
      data-tauri-drag-region
      className="flex h-8 shrink-0 cursor-move items-center gap-2 border-b border-edge bg-black pr-1 pl-3"
    >
      <span data-tauri-drag-region className="font-caps text-[13px] text-normal">
        poebo
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
