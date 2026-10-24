import { useEffect, useState } from 'react';

import {
  clearBuild,
  closeSettings,
  connectAgent,
  copyAssistantConfig,
  getAssistant,
  getPob,
  getPobSync,
  openPob,
  pickPobBuild,
  setPobSync,
  unpickPobBuild,
  getSettings,
  importBuildFile,
  pasteBuild,
  pickLogFile,
  pickPobFolder,
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
import type { AgentView, AssistantView, GemHints, Mule, PobInfo, PobSync, PresetStage } from '../ipc/types';
import { Button, Choice, ConfirmButton, Note, Row, Section, Select, Slider, Toggle } from '../kit';
import { useAppStore } from '../store/useAppStore';
import { HotkeySection } from './HotkeySection';
import { ReplayControls } from './ReplayControls';
import { Brand } from './Tooltip';

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
        <Slider
          value={textScale}
          min={0.8}
          max={1.5}
          step={0.05}
          label="Text size"
          onChange={(v) => {
            setTextScale(v);
            void updateSettings({ textScale: v });
          }}
        />
      </Row>
      <Row label="Background">
        <Slider
          value={opacity}
          min={0.7}
          max={1}
          step={0.05}
          label="Background opacity"
          onChange={(v) => {
            setOpacity(v);
            void updateSettings({ opacity: v });
          }}
        />
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
      <Note>
        Hides the overlay after that long without a new step. It comes back with the next area or
        step, or when you tap the menu key.
      </Note>
      <Row label="Position">
        <Button onClick={() => void snapOverlayBottomCenter()}>
          Bottom center
        </Button>
        <Button onClick={() => void startMove()}>
          Move overlay
        </Button>
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
        <Button onClick={() => void routePrev()}>
          Back
        </Button>
        <Button onClick={() => void routeNext()}>
          Next
        </Button>
        <ConfirmButton
          label="Start over"
          confirm="Click again to start over"
          onConfirm={() => void routeReset()}
        />
      </Row>
      <Note>
        The route is exile-leveling's (MIT), the speedrun route by HeartofPhos and contributors.
      </Note>
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
        <Button
          onClick={async () => {
            const picked = await pickLogFile();
            if (picked) await startLive(picked);
          }}
        >
          Pick Client.txt
        </Button>
      </Row>
      <Row label="Testing">
        {mode !== 'replay' && (
          <Button aria-expanded={showTesting} onClick={() => setTesting(!testing)}>
            {showTesting ? 'Hide' : 'Show'}
          </Button>
        )}
        {showTesting && (
          <>
            <Button
              onClick={async () => {
                const picked = await pickLogFile();
                if (picked) await startReplay(picked, 'X5');
              }}
            >
              Replay a log
            </Button>
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
        <Note>
          Plays a saved log back to check the route without the game. Stop leaves Exile Companion without a
          log until you pick one again.
        </Note>
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
      <Choice
        value={hints}
        options={GEM_HINTS}
        onChange={(v) => {
          setHints(v);
          void updateSettings({ gemHints: v });
        }}
      />
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

function stageOptions(autoLevel: number | null): { value: string; label: string }[] {
  return [
    { value: 'auto', label: autoLabel(autoLevel) },
    ...ACTS.map((a) => ({ value: `act${a}`, label: `From act ${a}` })),
    { value: 'endgame', label: 'Endgame' },
    { value: 'skip', label: 'Leave out' },
  ];
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
            <Select
              className="w-40"
              label={`Stage of ${p.title || `preset ${i + 1}`}`}
              value={stageValue(stages[i])}
              options={stageOptions(p.auto_level)}
              onChange={(v) => change(i, stageFrom(v))}
            />
          )}
        </Row>
      ))}
      <Note>
        A gem is planned from the first preset it shows up in. Auto reads the act or level from the
        preset name and keeps the order. Name presets like Act 3 or lvl 28 and this is rarely needed.
      </Note>
    </>
  );
}

// The build assistant runs the player's own Path of Building, so it calculates what PoB does.
function PobRows(): React.JSX.Element {
  const hasBuild = useAppStore((s) => s.build !== null);
  const [pob, setPob] = useState<PobInfo | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void getPob().then(setPob);
  }, []);
  return (
    <>
      <Row label="PoB folder">
        {/* Cut from the left like the Client.txt path, the folder name is the part that tells. */}
        <span
          dir="rtl"
          className="min-w-0 flex-1 truncate text-left font-body text-[13px] text-value"
          title={pob?.path ?? ''}
        >
          <bdi>{pob === undefined ? 'Looking…' : (pob?.path ?? 'Not found')}</bdi>
        </span>
        {pob?.version && (
          <span className="shrink-0 font-body text-[13px] text-label">{pob.version}</span>
        )}
      </Row>
      <Row label="">
        <Button
          onClick={async () => {
            setError(null);
            try {
              const picked = await pickPobFolder();
              if (picked) setPob(picked);
            } catch (e) {
              setError(String(e));
            }
          }}
        >
          Pick PoB folder
        </Button>
      </Row>
      {error && (
        <Row label="">
          <span role="alert" className="min-w-0 flex-1 font-body text-[12px] text-corrupted">
            {error}
          </span>
        </Row>
      )}
      <Note>
        The build assistant will calculate with your Path of Building. Exile Companion looks for it
        in the usual install folders. Pick the folder if yours is somewhere else.
      </Note>
      {/* Clearing the build turns sync off, so the rows load again when it goes. */}
      {pob && <PobSyncRows key={hasBuild ? 'build' : 'none'} />}
    </>
  );
}

// PoB lists builds by file name, so that's what the row shows.
function buildName(file: string): string {
  return file.split(/[\\/]/).pop()?.replace(/\.xml$/i, '') ?? file;
}

function PobSyncRows(): React.JSX.Element {
  const [sync, setSync] = useState<PobSync | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void getPobSync().then(setSync);
  }, []);
  const run = async (action: () => Promise<PobSync | null | void>): Promise<void> => {
    setError(null);
    try {
      const next = await action();
      if (next) setSync(next);
    } catch (e) {
      setError(String(e));
    }
  };
  if (!sync) return <></>;
  return (
    <>
      <Row label="Sync with PoB">
        <Toggle on={sync.enabled} label="Sync with PoB" onChange={(v) => void run(() => setPobSync(v))} />
      </Row>
      {sync.enabled && sync.file && (
        <Row label="PoB build">
          <span className="min-w-0 flex-1 truncate font-body text-[13px] text-value" title={sync.file}>
            {buildName(sync.file)}
          </span>
        </Row>
      )}
      <Row label="">
        <Button onClick={() => void run(pickPobBuild)}>Pick PoB build</Button>
        {sync.enabled && sync.picked && (
          <Button onClick={() => void run(unpickPobBuild)}>Use Exile Companion</Button>
        )}
        <Button onClick={() => void run(openPob)}>Open PoB</Button>
      </Row>
      {error && (
        <Row label="">
          <span role="alert" className="min-w-0 flex-1 font-body text-[12px] text-corrupted">
            {error}
          </span>
        </Row>
      )}
      <Note>
        Keeps your build here and in Path of Building the same, whichever you changed last. PoB
        lists it as Exile Companion. Pick the build you import your character into instead, and it
        replaces the build here. Reopen the build in PoB after the assistant changes it.
      </Note>
    </>
  );
}

function muleText(m: Mule): string {
  const parts = [];
  if (m.take) parts.push(`take ${m.take.name}`);
  if (m.buy.length > 0) {
    parts.push(`buy ${m.buy.map((g) => g.name).join(', ')} from ${m.buy[0].npc}`);
  }
  return `${m.class}: ${parts.join(', ')}`;
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
        <Button
          onClick={async () => {
            const s = await importBuildFile();
            if (s) setBuild(s);
          }}
        >
          Import file
        </Button>
        <Button
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
        </Button>
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
      <PobRows />
      <GemHintsRow />
      {build && build.not_in_campaign.length > 0 && (
        <Row label="Not in campaign">
          <span className="min-w-0 flex-1 font-body text-[13px] text-value">
            {build.not_in_campaign.join(', ')}
          </span>
        </Row>
      )}
      {build && build.mules.length > 0 && (
        <>
          {build.mules.map((m, i) => (
            <Row key={i} label={i === 0 ? 'Mule first' : ''}>
              <span className="min-w-0 flex-1 font-body text-[13px] text-value">{muleText(m)}</span>
            </Row>
          ))}
          <Note>
            Your class gets these gems only later, or never. Before you start, make a character of that
            class in the same league, kill Hillock, and leave the gems in the stash. It takes a couple
            of minutes.
          </Note>
        </>
      )}
      <PresetRows />
    </Section>
  );
}

function agentState(agent: AgentView): { text: string; className: string } {
  switch (agent.status.state) {
    case 'connected':
      return { text: 'Connected', className: 'text-quest' };
    case 'otherCopy':
      return { text: 'Set to another copy', className: 'text-value' };
    case 'notConnected':
      return { text: 'Not connected', className: 'text-value' };
    default:
      return { text: 'Not installed', className: 'text-label' };
  }
}

// The assistant is an MCP server that the player's own AI agent starts. Connecting writes it
// into the agent's config, so the player never edits JSON by hand.
function AssistantSection(): React.JSX.Element {
  const [view, setView] = useState<AssistantView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    void getAssistant().then(setView);
  }, []);
  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(t);
  }, [copied]);

  if (view && !view.server) {
    return (
      <Section title="Assistant">
        <Note flush>This build of Exile Companion doesn't include the assistant.</Note>
      </Section>
    );
  }
  return (
    <Section title="Assistant">
      <Note flush>
        Ask Claude or another AI agent about your build. It calculates with your Path of Building and
        reads the Path of Exile Wiki. Connect your agent, restart it, and ask in its chat.
      </Note>
      {(view?.agents ?? []).map((agent) => {
        const state = agentState(agent);
        return (
          <Row key={agent.id} label={agent.name}>
            <span className={`min-w-0 flex-1 truncate font-body text-[13px] ${state.className}`} title={agent.config}>
              {state.text}
            </span>
            {(agent.status.state === 'notConnected' || agent.status.state === 'otherCopy') && (
              <Button
                aria-label={`Connect ${agent.name}`}
                onClick={async () => {
                  setError(null);
                  try {
                    setView(await connectAgent(agent.id));
                  } catch (e) {
                    setError(String(e));
                  }
                }}
              >
                Connect
              </Button>
            )}
          </Row>
        );
      })}
      {error && (
        <Row label="">
          <span role="alert" className="min-w-0 flex-1 font-body text-[12px] text-corrupted">
            {error}
          </span>
        </Row>
      )}
      <Row label="Other agents">
        <Button
          onClick={async () => {
            setError(null);
            try {
              await copyAssistantConfig();
              setCopied(true);
            } catch (e) {
              setError(String(e));
            }
          }}
        >
          {copied ? 'Copied' : 'Copy config'}
        </Button>
      </Row>
      <Note>Paste it where your agent keeps its MCP servers. Most agents read this format.</Note>
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
      <Button
        className="ml-auto grid size-6 cursor-default place-items-center p-0"
        aria-label="Close settings"
        onClick={() => void closeSettings()}
      >
        <svg viewBox="0 0 12 12" className="size-3" aria-hidden="true">
          <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" fill="none" stroke="currentColor" strokeWidth="1.4" />
        </svg>
      </Button>
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
          <AssistantSection />
          <HotkeysGroup />
        </main>
      </div>
    </div>
  );
}
