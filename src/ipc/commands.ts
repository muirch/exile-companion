// Typed wrappers over the Tauri command surface (`poebo_app::commands`).
//
// In a plain browser (`pnpm dev`, no Tauri) these fall back to the in-browser mock so the UI is
// fully explorable without a backend.

import { invoke } from '@tauri-apps/api/core';
import { readText } from '@tauri-apps/plugin-clipboard-manager';

import { isTauri } from './env';
import * as mock from './mock';
import type {
  BuildSummary,
  CampaignState,
  NextObjective,
  Settings,
  SpeedMultiplier,
} from './types';

const EMPTY_STATE: CampaignState = {
  character: null,
  class: null,
  level: 0,
  act: null,
  zone: null,
  location_kind: 'unknown',
  deaths: 0,
  session_started: false,
};

export const pickLogFile = (): Promise<string | null> =>
  isTauri() ? invoke('pick_log_file') : Promise.resolve('examples/sample-client.txt');

export const startLive = (path: string): Promise<CampaignState> => {
  if (isTauri()) return invoke('start_live', { path });
  mock.restartMock();
  return Promise.resolve(EMPTY_STATE);
};

export const stopSource = (): Promise<void> => {
  if (isTauri()) return invoke('stop_source');
  mock.stopMock();
  return Promise.resolve();
};

export const startReplay = (path: string, speed: SpeedMultiplier): Promise<void> => {
  if (isTauri()) return invoke('start_replay', { path, speed });
  mock.setMockSpeed(speed);
  mock.restartMock();
  return Promise.resolve();
};

export const replayPause = (): Promise<void> => {
  if (isTauri()) return invoke('replay_pause');
  mock.pauseMock();
  return Promise.resolve();
};

export const replayResume = (): Promise<void> => {
  if (isTauri()) return invoke('replay_resume');
  mock.resumeMock();
  return Promise.resolve();
};

export const replayStep = (): Promise<void> => {
  if (isTauri()) return invoke('replay_step');
  mock.stepMock();
  return Promise.resolve();
};

export const replaySetSpeed = (speed: SpeedMultiplier): Promise<void> => {
  if (isTauri()) return invoke('replay_set_speed', { speed });
  mock.setMockSpeed(speed);
  return Promise.resolve();
};

export const getState = (): Promise<CampaignState> =>
  isTauri() ? invoke('get_state') : Promise.resolve(EMPTY_STATE);

/** Import a build from a local PoB file via a native picker. Resolves null if cancelled. */
export const importBuildFile = (): Promise<BuildSummary | null> =>
  isTauri() ? invoke('import_build_file') : Promise.resolve(mock.importBuildMock());

/** Import a build from a pasted PoB code or raw XML. */
export const importBuildCode = (code: string): Promise<BuildSummary> =>
  isTauri() ? invoke('import_build_code', { code }) : Promise.resolve(mock.importBuildMock());

/** Read the OS clipboard and import whatever PoB code/XML it holds. */
export const pasteBuild = async (): Promise<BuildSummary> => {
  if (!isTauri()) return mock.importBuildMock();
  const code = await readText();
  return invoke('import_build_code', { code });
};

export const clearBuild = (): Promise<void> => {
  if (isTauri()) return invoke('clear_build');
  mock.clearBuildMock();
  return Promise.resolve();
};

export const getBuild = (): Promise<BuildSummary | null> =>
  isTauri() ? invoke('get_build') : Promise.resolve(mock.getBuildMock());

export const getGuidance = (): Promise<NextObjective> =>
  isTauri() ? invoke('get_guidance') : Promise.resolve(mock.getGuidanceMock());

export const setClickThrough = (ignore: boolean): Promise<void> =>
  isTauri() ? invoke('set_click_through', { ignore }) : Promise.resolve();

export const getSettings = (): Promise<Settings> =>
  isTauri() ? invoke('get_settings') : Promise.resolve({ opacity: 0.85, lastLogPath: null });

export const saveSettings = (settings: Settings): Promise<void> =>
  isTauri() ? invoke('save_settings', { settings }) : Promise.resolve();
