/**
 * In-browser campaign simulator used only in `pnpm dev` (no Tauri backend).
 *
 * It plays a scripted run that mirrors examples/sample-client.txt into the store on a timer, so the
 * overlay animates through a realistic-looking campaign and can be styled without Windows or a VM.
 * The replay controls (pause / resume / step / speed) drive this simulator too.
 */

import type { CampaignState, LocationKind, SpeedMultiplier } from './types';
import { useAppStore } from '../store/useAppStore';

function s(
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
  };
}

// An Act 1 run into Act 2: level 1 -> 14, a death, town/combat, an act transition.
const SCRIPT: readonly CampaignState[] = [
  s(2, 1, 'The Twilight Strand', '1_1_1', 'combat', 0),
  s(2, 1, "Lioneye's Watch", '1_1_town', 'town', 0),
  s(3, 1, 'The Coast', '1_1_2', 'combat', 0),
  s(5, 1, 'The Submerged Passage', '1_1_4_1', 'combat', 0),
  s(7, 1, 'The Lower Prison', '1_1_7_1', 'combat', 0),
  s(7, 1, 'The Lower Prison', '1_1_7_1', 'combat', 1),
  s(9, 1, 'The Ship Graveyard', '1_1_9', 'combat', 1),
  s(11, 1, 'The Cavern of Anger', '1_1_11_2', 'combat', 1),
  s(11, 1, "Lioneye's Watch", '1_1_town', 'town', 1),
  s(11, 2, 'The Southern Forest', '1_2_1', 'combat', 1),
  s(12, 2, 'The Forest Encampment', '1_2_town', 'town', 1),
  s(13, 2, 'The Old Fields', '1_2_2', 'combat', 1),
  s(14, 2, 'The Crossroads', '1_2_5', 'combat', 1),
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
  useAppStore.getState().setCampaign(SCRIPT[index]);
  index += 1;
  pushProgress();
  if (!paused && index < SCRIPT.length) {
    timer = setTimeout(releaseNext, stepDelayMs());
  }
}

export function startMock(): void {
  useAppStore.getState().setSourceStatus({
    mode: 'replay',
    path: 'examples/sample-client.txt (mock)',
  });
  restartMock();
}

export function stopMock(): void {
  clearTimer();
}

export function restartMock(): void {
  clearTimer();
  index = 0;
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
  useAppStore.getState().setCampaign(SCRIPT[index]);
  index += 1;
  pushProgress();
}

export function setMockSpeed(speed: SpeedMultiplier): void {
  factor = Number(speed.slice(1));
  pushProgress();
}
