// Central IPC type surface: the ts-rs-generated Rust types plus a few hand-written shapes for
// commands/events whose Rust side lives in `app/desktop` (regenerate the generated/ dir with
// `pnpm gen:bindings`).

import type { BuildSummary } from './generated/BuildSummary';
import type { PresetStage } from './generated/PresetStage';
import type { SpeedMultiplier } from './generated/SpeedMultiplier';

export type { CampaignState } from './generated/CampaignState';
export type { ZoneRef } from './generated/ZoneRef';
export type { LocationKind } from './generated/LocationKind';
export type { LogEvent } from './generated/LogEvent';
export type { DeathCause } from './generated/DeathCause';
export type { SpeedMultiplier } from './generated/SpeedMultiplier';
export type { BuildSummary } from './generated/BuildSummary';
export type { Mule } from './generated/Mule';
export type { MuleGem } from './generated/MuleGem';
export type { PresetStage } from './generated/PresetStage';
export type { NextObjective } from './generated/NextObjective';
export type { ObjectiveKind } from './generated/ObjectiveKind';
export type { Part } from './generated/Part';
export type { Step } from './generated/Step';
export type { RouteView } from './generated/RouteView';

export type SourceMode = 'idle' | 'live' | 'replay';

/** Mirrors `exile_core::links`: a skill set's socket groups as build sites draw them. */
export type GemColor = 'red' | 'green' | 'blue' | 'white';

export interface LinkGem {
  name: string;
  color: GemColor;
  support: boolean;
  /** The passive tree or an item gives the skill, not a gem. */
  granted: boolean;
  level: number | null;
  quality: number | null;
  requiredLevel: number | null;
  /** "Quest reward from Tarkleigh in act 1", a mule, or not in the campaign. */
  get: string | null;
}

export interface LinkGroup {
  slot: string | null;
  label: string | null;
  gems: LinkGem[];
}

export interface Links {
  sets: string[];
  set: number;
  groups: LinkGroup[];
}

/** Mirrors `exile_core::tree`: PoB's passive tree and what a guide's stage marks on it. */
export type NodeKind = 'normal' | 'notable' | 'keystone' | 'jewel' | 'mastery' | 'ascendancyStart';

export interface TreeNode {
  id: number;
  x: number;
  y: number;
  kind: NodeKind;
  name: string;
  stats: string[];
  /** The ascendancy or bloodline the node belongs to. Missing on the main tree. */
  ascendancy?: string;
  /** A mastery's effects to pick from. */
  mastery?: { effect: number; stats: string[] }[];
  /** The game's explanation in brackets, "(Recently refers to the past 4 seconds)". */
  reminder?: string[];
  flavour?: string[];
}

export interface TreeEdge {
  a: number;
  b: number;
  /** The orbit's radius when the edge runs along it, from `a` to `b`. */
  arc: number | null;
  clockwise: boolean;
}

export interface TreeLayout {
  nodes: TreeNode[];
  edges: TreeEdge[];
  /** The name the game shows for an ascendancy or bloodline id, "Farrul" is "Farrul Bloodline". */
  ascendancyNames: Record<string, string>;
}

export type TreeMode = 'tree' | 'new' | 'character';
/** Added is new at the stage or still to take, Extra refunded or taken beyond the guide. */
export type Mark = 'taken' | 'added' | 'extra';

/** Mirrors `exile_core::gear::ItemView`: a build's item as the game's tooltip shows it. */
export interface ItemView {
  rarity: 'normal' | 'magic' | 'rare' | 'unique' | 'relic';
  name: string;
  base: string | null;
  quality: number | null;
  sockets: string | null;
  levelReq: number | null;
  radius: string | null;
  corrupted: boolean;
  mods: { text: string; kind: 'enchant' | 'implicit' | 'explicit' | 'crafted' | 'fractured' }[];
}

/** Mirrors `exile_core::tree::NodeMark`. */
export interface NodeMark {
  id: number;
  mark: Mark;
  since: number | null;
  /** The picked mastery effect. */
  effect: number | null;
  /** Taken by the amulet's anoint, not with a point. */
  anointed: boolean;
  tattoo: { name: string; stats: string[] } | null;
}

/** Mirrors `exile_core::tree::JewelMark`. */
export interface JewelMark {
  socket: number;
  item: ItemView;
  /** The node the area is around: the socket, or Impossible Escape's keystone. */
  centre: number;
  /** In tree units, null without an area. */
  radius: number | null;
  /** Nodes in the area need no path. */
  withoutPath: boolean;
}

export interface Bounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

export interface TreeStage {
  stages: string[];
  stage: number;
  version: string | null;
  /** The mode shown. Character falls back to tree without a character. */
  mode: TreeMode;
  canCompare: boolean;
  points: number;
  notables: number;
  added: number;
  extra: number;
  addedNames: string[];
  ascendancy: { name: string; mark: Mark }[];
  /** The ascendancies to draw: the guide's own, then a bloodline the stage takes. */
  ascendancies: string[];
  /** `effect` is the picked mastery effect. */
  nodes: NodeMark[];
  /** `edge` indexes `TreeLayout.edges`. */
  edges: { edge: number; mark: Mark }[];
  focus: Bounds | null;
  /** The newest tree in PoB when the stage is for an older one, "3_30". */
  latest: string | null;
  /** The compared character's tree version when it isn't the stage's. */
  characterVersion: string | null;
  /** Nodes of the stage or the compared tree the stage's tree data doesn't have. */
  missing: number;
  /** Their notables and keystones, named from the character's tree. */
  missingNames: string[];
  jewels: JewelMark[];
}

/** Mirrors `settings::TreePrefs`. */
export interface TreePrefs {
  /** Ascendancies and bloodlines folded to a bar, by id. */
  folded: string[];
  /** The list of keystones, ascendancy notables and masteries next to the tree. */
  list: boolean;
}

/** The followed guide (mirrors `runtime::GuideView`). */
export interface GuideView {
  summary: BuildSummary;
  source: string | null;
}

/** A character remembered from pathofexile.com (mirrors `exile_core::character::CharacterLink`). */
export interface CharacterLink {
  account: string;
  character: string;
  realm: string;
  imported: string;
}

/** The guide and the player's own build (mirrors `profile::Profile`). */
export interface Profile {
  guide: GuideView | null;
  character: BuildSummary | null;
  link: CharacterLink | null;
}

export interface CharacterEntry {
  name: string;
  class: string;
  level: number;
  league: string;
}

/** Mirrors `profile::CharacterList`. */
export interface CharacterList {
  account: string;
  characters: CharacterEntry[];
  playing: string | null;
}

/** Persisted user settings (mirrors `exile_companion::settings::Settings`). */
export interface Settings {
  opacity: number;
  lastLogPath: string | null;
  routeEdge: number;
  textScale: number;
  leagueStart: boolean;
  bandit: Bandit;
  gemHints: GemHints;
  compact: boolean;
  /** Seconds without a route change before the overlay fades out; 0 keeps it shown. */
  fadeAfter: number;
  presetStages: PresetStage[];
  pobPath: string | null;
  pobSync: boolean;
  pobBuild: string | null;
  chat: ChatConfig;
}

export type ChatProvider = 'anthropic' | 'openRouter' | 'custom';
export type ChatModel = 'claude-haiku-5-5' | 'claude-sonnet-5-5' | 'claude-opus-5-5';
export type ChatEffort = 'low' | 'medium' | 'high';

/** Mirrors `exile_assistant::Config`. */
export interface ChatConfig {
  provider: ChatProvider;
  /** The Claude model, for Anthropic. */
  model: ChatModel;
  effort: ChatEffort;
  routerModel: string;
  customUrl: string;
  customModel: string;
}

/** Mirrors `chat::ChatView`. */
export interface ChatView {
  /** Which providers have a key stored. */
  keys: Record<ChatProvider, boolean>;
  /** False in a build without the assistant's tools. */
  available: boolean;
  busy: boolean;
  /** What the player still has to set before the picked provider can answer. */
  missing: string | null;
  /** The picked model, as the player named it. */
  model: string;
}

/** Mirrors `exile_assistant::NextCard`: what to change next, worked out without a model. */
export interface NextCard {
  /** Why there are no steps, in the player's words. */
  missing: string | null;
  stage: string | null;
  laterStage: string | null;
  steps: NextStep[];
}

export interface NextStep {
  kind: 'gem' | 'passive' | 'gear' | 'ascendancy';
  name: string;
  slot: string | null;
  points: number | null;
  dpsPct: number;
  ehpPct: number;
}

/** Mirrors `exile_assistant::GearCard`: the best slot for a copied item, measured with PoB. */
export interface GearCard {
  slot: string;
  replaces: string | null;
  verdict: 'upgrade' | 'downgrade' | 'mixed' | 'same';
  dpsPct: number;
  ehpPct: number;
  life: number;
  resists: { element: string; change: number; after: number }[];
  /** PoB couldn't calculate some of the item, so the numbers leave it out. */
  partly: boolean;
}

/** Mirrors `exile_core::item_check::GemCard`. */
export interface GemCard {
  name: string;
  uses: { set: string | null; skill: string; links: string[] }[];
  get: string | null;
}

/** Mirrors `exile_core::gemplan::GemOverride`. */
export type GemOverride = { act: number } | 'have';

/** Mirrors `runtime::GemRow`: a gem of the build in the settings. */
export interface GemRow {
  name: string;
  /** Where the plan takes it. Null when the campaign doesn't give it or the player has it. */
  place: string | null;
  over: GemOverride | null;
}

/** Payload of `item://card`, mirrors `item::ItemCard`. */
export type ItemCard =
  | { kind: 'measuring'; name: string }
  | ({ kind: 'gem' } & GemCard)
  | { kind: 'gear'; name: string; card: GearCard; needsLevel: number | null; guideGear: boolean }
  | { kind: 'missing'; name: string; message: string }
  | { kind: 'ready'; level: number; gems: string[]; items: { name: string; slot: string; level: number }[] }
  | { kind: 'kitava'; penalty: number; short: { element: string; after: number }[] }
  | { kind: 'next'; act: number; card: NextCard };

/** Payload of `chat://event`, mirrors `exile_assistant::ChatEvent`. */
export type ChatEvent =
  | { kind: 'text'; text: string }
  | { kind: 'paragraph' }
  | { kind: 'note'; text: string }
  | { kind: 'tool'; name: string };

export type ChatStop = 'done' | 'cut' | 'refused' | 'tooManySteps' | 'empty' | 'stopped' | 'error';

/** A saved chat in the history list (mirrors `exile_assistant::history::ChatSummary`). */
export interface ChatSummary {
  id: string;
  title: string;
  /** Unix seconds of the last question. */
  updated: number;
  model: string;
  questions: number;
  dollars: number | null;
}

/** Mirrors `exile_assistant::history::SavedExchange`. */
export interface SavedExchange {
  question: string;
  answer: string;
  stop: ChatStop;
  message: string | null;
  dollars: number | null;
  model: string | null;
}

/** Whether the provider still caches the chat (mirrors `exile_assistant::history::Warmth`). */
export type Warmth = { kind: 'warm' } | { kind: 'cold'; tokens: number; dollars: number | null };

/** A saved chat opened for reading (mirrors `chat::OpenedChat`). */
export interface OpenedChat {
  id: string;
  title: string;
  exchanges: SavedExchange[];
  warmth: Warmth;
  /** Why the picked provider can't go on with it. */
  blocked: string | null;
}

/** Payload of `chat://over`, mirrors `chat::TurnOver`. */
export interface TurnOver {
  stop: ChatStop;
  /** The error for the player, or the refusal's category. */
  message: string | null;
  usage: {
    inputTokens: number;
    outputTokens: number;
    cacheReadTokens: number;
    cacheWriteTokens: number;
    cost: number | null;
  } | null;
  /** Null when the provider didn't say what the answer cost. */
  dollars: number | null;
  /** The model that answered. */
  model: string | null;
}

/** Which PoB build file the app's build is kept the same as (mirrors `pob::PobSync`). */
export interface PobSync {
  enabled: boolean;
  /** Null when PoB isn't found. */
  file: string | null;
  /** The player picked the file, it isn't Exile Companion's own. */
  picked: boolean;
}

/** Path of Building's folder and the version from its manifest (mirrors `pob::PobInfo`). */
export interface PobInfo {
  path: string;
  version: string | null;
}

/** What the player does with the act 2 bandits (mirrors `exile_core::route::Bandit`). */
export type Bandit = 'killAll' | 'alira' | 'kraityn' | 'oak';

/** Which gem steps the route shows (mirrors `exile_core::gemplan::GemHints`). */
export type GemHints = 'off' | 'rewards' | 'rewardsAndVendors';

/** Payload of `source://status`. */
export interface SourceStatus {
  mode: SourceMode;
  path?: string | null;
  error?: string | null;
}

/** Payload of `replay://progress`. */
export interface ReplayProgress {
  position: number;
  total: number;
  speed: SpeedMultiplier;
  paused: boolean;
}

/** Mirrors `exile_companion::hotkeys::Hotkeys`: global-hotkey syntax such as "Alt+ArrowLeft". */
export interface Hotkeys {
  menuHold: string;
  moveOverlay: string;
  routePrev: string;
  routeNext: string;
  relog: string;
  openChat: string;
  vendorSearch: string;
  hideout: string;
  itemCheck: string;
}

export interface HotkeySettings {
  hotkeys: Hotkeys;
  relogEnabled: boolean;
  hideoutEnabled: boolean;
  itemCheckEnabled: boolean;
  vendorSearchEnabled: boolean;
}

export interface BindError {
  action: string;
  shortcut: string;
  error: string;
}

/** Mirrors `exile_core::agents::Status`. */
export type AgentStatus =
  | { state: 'notFound' }
  | { state: 'notConnected' }
  | { state: 'connected' }
  | { state: 'otherCopy'; command: string };

export type AgentId = 'claudeDesktop' | 'claudeCode' | 'cursor' | 'vsCode' | 'codex';

/** Mirrors `assistant::AgentView`. */
export interface AgentView {
  id: AgentId;
  name: string;
  status: AgentStatus;
  config: string;
}

/** Mirrors `assistant::AssistantView`. */
export interface AssistantView {
  server: string | null;
  agents: AgentView[];
}
