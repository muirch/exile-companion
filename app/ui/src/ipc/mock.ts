/**
 * In-browser campaign simulator used only in `pnpm dev` (no Tauri backend).
 *
 * It plays a scripted run that mirrors docs/sample-client.txt into the store on a timer, so the
 * overlay animates through a realistic-looking campaign and can be styled without Windows or a VM.
 * It also drives a fake imported build and a per-step "next objective" (the real ones come from the
 * Rust GuidanceEngine under Tauri). The replay controls (pause / resume / step / speed) drive this
 * simulator too.
 */

import type {
  BuildSummary,
  CampaignState,
  ChatEvent,
  TurnOver,
  LocationKind,
  NextObjective,
  RouteView,
  SpeedMultiplier,
} from './types';
import type { Exchange } from '../components/chatLog';
import { useAppStore } from '../store/useAppStore';
// Real views from the bundled route (`cargo run -p exile-core --example dump_route`).
import MOCK_ROUTE from './mock-route.json';

interface MockView {
  view: RouteView;
  zone: string | null;
  act: number | null;
  town: boolean;
}
const ROUTE = MOCK_ROUTE as MockView[];
let routeIndex = 0;

/** A fake imported build, shown once "imported" in dev mode. */
const MOCK_BUILD: BuildSummary = {
  class_name: 'Witch',
  ascend_class_name: 'Necromancer',
  level: 90,
  active_spec_title: 'Leveling (Act 1)',
  main_skill: 'Raise Zombie',
  spec_count: 3,
  gem_count: 24,
  not_in_campaign: ['Vaal Haste', 'Empower Support'],
  mules: [
    {
      class: 'Shadow',
      take: { name: 'Cobra Lash', npc: 'Tarkleigh', own_act: 3 },
      buy: [{ name: 'Momentum', npc: 'Nessa', own_act: 3 }],
    },
  ],
  presets: [
    { title: 'Leveling (Act 1)', gem_count: 6, auto_level: 1 },
    { title: '----- Mapping -----', gem_count: 0, auto_level: 100 },
    { title: 'Mapping', gem_count: 18, auto_level: 100 },
  ],
};

interface MockStep {
  state: CampaignState;
  objective: NextObjective;
}

function state(
  level: number,
  act: number,
  zoneName: string,
  zoneId: string,
  location_kind: LocationKind,
  deaths: number,
): CampaignState {
  return {
    character: 'Xophina',
    class: 'Witch',
    level,
    act,
    zone: { name: zoneName, id: zoneId, act },
    location_kind,
    deaths,
    session_started: true,
    area_id: zoneId,
  };
}

function obj(kind: NextObjective['kind'], headline: string, detail: string | null): NextObjective {
  return { kind, headline, detail };
}

// An Act 1 run into Act 2 with a varied set of objectives, one per step, exercising every kind.
const SCRIPT: readonly MockStep[] = [
  {
    state: state(2, 1, 'The Twilight Strand', '1_1_1', 'combat', 0),
    objective: obj('proceed_to_zone', 'Head to The Coast', 'Act 1'),
  },
  {
    state: state(2, 1, "Lioneye's Watch", '1_1_town', 'town', 0),
    objective: obj('quest_reward', 'Claim Raise Zombie from Tarkleigh', 'Enemy at the Gate reward (Act 1)'),
  },
  {
    state: state(3, 1, 'The Coast', '1_1_2', 'combat', 0),
    objective: obj('proceed_to_zone', 'Head to The Tidal Island', 'Act 1'),
  },
  {
    state: state(5, 1, 'The Submerged Passage', '1_1_4_1', 'combat', 0),
    objective: obj('proceed_to_zone', 'Head to The Ledge', 'Act 1'),
  },
  {
    state: state(7, 1, 'The Lower Prison', '1_1_7_1', 'combat', 0),
    objective: obj('lab_trial', 'Trial of Ascendancy: The Lower Prison', 'Act 1 lab trial'),
  },
  {
    state: state(7, 1, 'The Lower Prison', '1_1_7_1', 'combat', 1),
    objective: obj('lab_trial', 'Trial of Ascendancy: The Lower Prison', 'Act 1 lab trial'),
  },
  {
    state: state(9, 1, 'The Ship Graveyard', '1_1_9', 'combat', 1),
    objective: obj('proceed_to_zone', 'Head to The Cavern of Wrath', 'Act 1'),
  },
  {
    state: state(10, 1, "Lioneye's Watch", '1_1_town', 'town', 1),
    objective: obj('passive_points', '1 passive point in Act 1', 'The Way Forward'),
  },
  {
    state: state(11, 1, 'The Cavern of Anger', '1_1_11_2', 'combat', 1),
    objective: obj('build_spec', 'Build tree: Leveling (Act 1)', 'Main skill: Raise Zombie'),
  },
  {
    state: state(11, 1, "Lioneye's Watch", '1_1_town', 'town', 1),
    objective: obj('quest_reward', 'Claim Spell Echo Support from Nessa', 'The Caged Brute reward (Act 1)'),
  },
  {
    state: state(11, 2, 'The Southern Forest', '1_2_1', 'combat', 1),
    objective: obj('proceed_to_zone', 'Head to The Old Fields', 'Act 2'),
  },
  {
    state: state(12, 2, 'The Forest Encampment', '1_2_town', 'town', 1),
    objective: obj('quest_reward', 'Claim Herald of Ash from Greust (+1 more)', 'Intruders in Black reward (Act 2)'),
  },
  {
    state: state(13, 2, 'The Old Fields', '1_2_2', 'combat', 1),
    objective: obj('proceed_to_zone', 'Head to The Crossroads', 'Act 2'),
  },
  {
    state: state(14, 2, 'The Crossroads', '1_2_5', 'combat', 1),
    objective: obj('proceed_to_zone', 'Head to The Broken Bridge', 'Act 2'),
  },
];

const SPEEDS: readonly SpeedMultiplier[] = ['X1', 'X2', 'X5', 'X20', 'X100'];

let timer: ReturnType<typeof setTimeout> | undefined;
let index = 0;
let paused = false;
let factor = 5;

function speedTag(): SpeedMultiplier {
  return SPEEDS.find((sm) => Number(sm.slice(1)) === factor) ?? 'X5';
}

function stepDelayMs(): number {
  return Math.max(120, Math.round(1500 / factor));
}

function pushProgress(): void {
  useAppStore.getState().setReplayProgress({
    position: Math.min(index, SCRIPT.length),
    total: SCRIPT.length,
    speed: speedTag(),
    paused,
  });
}

function applyStep(step: MockStep): void {
  useAppStore.getState().setCampaign(step.state);
  useAppStore.getState().setGuidance(step.objective);
  moveRouteMock(1);
}

function clearTimer(): void {
  if (timer !== undefined) {
    clearTimeout(timer);
    timer = undefined;
  }
}

function releaseNext(): void {
  timer = undefined;
  if (index >= SCRIPT.length) {
    pushProgress();
    return;
  }
  applyStep(SCRIPT[index]);
  index += 1;
  pushProgress();
  if (!paused && index < SCRIPT.length) {
    timer = setTimeout(releaseNext, stepDelayMs());
  }
}

export function startMock(): void {
  useAppStore.getState().setSourceStatus({
    mode: 'replay',
    path: 'C:\\Program Files (x86)\\Steam\\steamapps\\common\\Path of Exile\\logs\\Client.txt',
  });
  // Pretend a build is already imported so the build + guidance panels are populated in dev.
  useAppStore.getState().setBuild(MOCK_BUILD);
  // ?view=locked or ?view=notice previews the in-game and startup states.
  const view = new URLSearchParams(window.location.search).get('view');
  useAppStore.getState().setGrabbed(view !== 'locked' && view !== 'notice');
  useAppStore.getState().setGameFocused(view !== 'notice');
  useAppStore.getState().setCompact(new URLSearchParams(window.location.search).has('compact'));
  restartMock();
  // ?edge=N freezes the mock on that route view.
  const edge = Number(new URLSearchParams(window.location.search).get('edge'));
  if (edge > 0) {
    pauseMock();
    moveRouteMock(edge);
  }
}

export function stopMock(): void {
  clearTimer();
}

export function restartMock(): void {
  clearTimer();
  index = 0;
  routeIndex = -1;
  paused = false;
  releaseNext();
}

export function pauseMock(): void {
  paused = true;
  clearTimer();
  pushProgress();
}

export function resumeMock(): void {
  if (!paused) return;
  paused = false;
  releaseNext();
}

export function stepMock(): void {
  if (!paused || index >= SCRIPT.length) return;
  applyStep(SCRIPT[index]);
  index += 1;
  pushProgress();
}

export function setMockSpeed(speed: SpeedMultiplier): void {
  factor = Number(speed.slice(1));
  pushProgress();
}

// --- Build import (dev-mode stand-ins for the Rust import commands) ---

export function importBuildMock(): BuildSummary {
  useAppStore.getState().setBuild(MOCK_BUILD);
  return MOCK_BUILD;
}

export function clearBuildMock(): void {
  useAppStore.getState().setBuild(null);
}

export function getBuildMock(): BuildSummary | null {
  return useAppStore.getState().build;
}

export function getGuidanceMock(): NextObjective {
  return (
    useAppStore.getState().guidance ??
    obj('idle', 'Import a Path of Building build', 'Load a build for gem and tree guidance')
  );
}

export function routeMock(): RouteView | null {
  return ROUTE[routeIndex]?.view ?? null;
}

// Keeps the header on the zone the shown steps belong to.
export function moveRouteMock(delta: number): RouteView | null {
  routeIndex = Math.min(ROUTE.length - 1, Math.max(0, routeIndex + delta));
  const entry = ROUTE[routeIndex];
  const st = useAppStore.getState();
  st.setRoute(entry?.view ?? null);
  if (entry?.zone) {
    st.setCampaign({
      ...st.campaign,
      zone: { name: entry.zone, id: entry.view.area_id, act: entry.act },
      act: entry.act,
      area_id: entry.view.area_id,
      location_kind: entry.town ? 'town' : 'combat',
    });
  }
  return entry?.view ?? null;
}

// The chat without a backend: one canned answer, streamed the way the real one arrives.
type ChatListener = { event?: (e: ChatEvent) => void; over?: (o: TurnOver) => void };
const chatListeners = new Set<ChatListener>();
let chatTimers: ReturnType<typeof setTimeout>[] = [];

export function onChatMock(listener: ChatListener): () => void {
  chatListeners.add(listener);
  return () => chatListeners.delete(listener);
}

const emitChat = (e: ChatEvent): void => chatListeners.forEach((l) => l.event?.(e));
const overChat = (o: TurnOver): void => chatListeners.forEach((l) => l.over?.(o));

export const MOCK_ANSWER =
  'Take Herbalism next, 2 points from Thick Skin.\n\n' +
  'It is the biggest life gain on your way to the guide\'s level 40 tree.\n' +
  '- +12% maximum life, about 180 life at your level\n' +
  '- the guide takes it at 38, so you are on time\n\n' +
  'After that, Arcane Will for mana regeneration.';

export function chatMock(question: string): Promise<void> {
  chatStopMock();
  void question;
  const steps: [number, () => void][] = [
    [100, () => emitChat({ kind: 'tool', name: 'player_status' })],
    [400, () => emitChat({ kind: 'note', text: 'Comparing your tree with the guide at level 40.' })],
    [700, () => emitChat({ kind: 'tool', name: 'passive_value' })],
  ];
  const words = MOCK_ANSWER.split(/(?<= )/);
  words.forEach((w, i) => steps.push([1100 + i * 15, () => emitChat({ kind: 'text', text: w })]));
  steps.push([
    1150 + words.length * 15,
    () =>
      overChat({
        stop: 'done',
        message: null,
        usage: { inputTokens: 900, outputTokens: 420, cacheReadTokens: 31000, cacheWriteTokens: 1200 },
        dollars: 0.0191,
      }),
  ]);
  chatTimers = steps.map(([at, f]) => setTimeout(f, at));
  return Promise.resolve();
}

export function chatStopMock(): void {
  const running = chatTimers.length > 0;
  chatTimers.forEach(clearTimeout);
  chatTimers = [];
  if (running) overChat({ stop: 'stopped', message: null, usage: null, dollars: null });
}

const chatQuery = (): string | null =>
  typeof window === 'undefined' ? null : new URLSearchParams(window.location.search).get('chat');

export const chatHasKeyMock = (): boolean => chatQuery() !== 'nokey';

const PASTED_ITEM = `Item Class: Body Armours
Rarity: Rare
Grim Shelter
Leather Belt
--------
+38 to maximum Life
+24% to Fire Resistance`;

/** What the chat window shows on load in the browser, picked with ?chat=. */
export function chatSeedMock(): Exchange[] {
  if (chatQuery() !== 'answered') return [];
  return [
    {
      question: 'Which passive should I take next?',
      answer: MOCK_ANSWER,
      tool: null,
      note: null,
      over: { stop: 'done', message: null, usage: null, dollars: 0.0191 },
    },
    {
      question: PASTED_ITEM,
      answer: '',
      tool: 'compare_item',
      note: 'Your chest has 21% less life than this one would give.',
      over: null,
    },
  ];
}
