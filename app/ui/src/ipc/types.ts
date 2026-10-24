// Central IPC type surface: the ts-rs-generated Rust types plus a few hand-written shapes for
// commands/events whose Rust side lives in `app/desktop` (regenerate the generated/ dir with
// `pnpm gen:bindings`).

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

/** Persisted user settings (mirrors `exile_companion::settings::Settings`). */
export interface Settings {
  opacity: number;
  lastLogPath: string | null;
  routeEdge: number;
  textScale: number;
  leagueStart: boolean;
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

export type ChatModel = 'claude-sonnet-5-5' | 'claude-opus-5-5';
export type ChatEffort = 'low' | 'medium' | 'high';

/** Mirrors `exile_assistant::Config`. */
export interface ChatConfig {
  model: ChatModel;
  effort: ChatEffort;
}

/** Mirrors `chat::ChatView`. */
export interface ChatView {
  hasKey: boolean;
  /** False in a build without the assistant's tools. */
  available: boolean;
  busy: boolean;
}

/** Payload of `chat://event`, mirrors `exile_assistant::ChatEvent`. */
export type ChatEvent =
  | { kind: 'text'; text: string }
  | { kind: 'paragraph' }
  | { kind: 'note'; text: string }
  | { kind: 'tool'; name: string };

export type ChatStop = 'done' | 'cut' | 'refused' | 'tooManySteps' | 'stopped' | 'error';

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
  } | null;
  dollars: number | null;
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
}

export interface HotkeySettings {
  hotkeys: Hotkeys;
  relogEnabled: boolean;
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
