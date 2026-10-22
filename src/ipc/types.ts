// Central IPC type surface: the ts-rs-generated Rust types plus a few hand-written shapes for
// commands/events whose Rust side lives in `src-tauri` (regenerate the generated/ dir with
// `pnpm gen:bindings`).

import type { SpeedMultiplier } from './generated/SpeedMultiplier';

export type { CampaignState } from './generated/CampaignState';
export type { ZoneRef } from './generated/ZoneRef';
export type { LocationKind } from './generated/LocationKind';
export type { LogEvent } from './generated/LogEvent';
export type { DeathCause } from './generated/DeathCause';
export type { SpeedMultiplier } from './generated/SpeedMultiplier';
export type { BuildSummary } from './generated/BuildSummary';
export type { NextObjective } from './generated/NextObjective';
export type { ObjectiveKind } from './generated/ObjectiveKind';
export type { Part } from './generated/Part';
export type { Step } from './generated/Step';
export type { RouteView } from './generated/RouteView';

export type SourceMode = 'idle' | 'live' | 'replay';

/** Persisted user settings (mirrors `poebo_app::settings::Settings`). */
export interface Settings {
  opacity: number;
  lastLogPath: string | null;
  routeEdge: number;
  textScale: number;
  leagueStart: boolean;
  gemHints: GemHints;
}

/** Which gem steps the route shows (mirrors `poebo_core::gemplan::GemHints`). */
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

/** Mirrors `poebo_app::hotkeys::Hotkeys`: global-hotkey syntax such as "Alt+ArrowLeft". */
export interface Hotkeys {
  menuHold: string;
  moveOverlay: string;
  routePrev: string;
  routeNext: string;
  relog: string;
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
