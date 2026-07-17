// Typed wrappers over the Tauri command surface (`poebo_app::commands`).

import { invoke } from '@tauri-apps/api/core';

import type { CampaignState, Settings, SpeedMultiplier } from './types';

export const pickLogFile = (): Promise<string | null> => invoke('pick_log_file');

export const startLive = (path: string): Promise<CampaignState> => invoke('start_live', { path });

export const stopSource = (): Promise<void> => invoke('stop_source');

export const startReplay = (path: string, speed: SpeedMultiplier): Promise<void> =>
  invoke('start_replay', { path, speed });

export const replayPause = (): Promise<void> => invoke('replay_pause');
export const replayResume = (): Promise<void> => invoke('replay_resume');
export const replayStep = (): Promise<void> => invoke('replay_step');
export const replaySetSpeed = (speed: SpeedMultiplier): Promise<void> =>
  invoke('replay_set_speed', { speed });

export const getState = (): Promise<CampaignState> => invoke('get_state');

export const setClickThrough = (ignore: boolean): Promise<void> =>
  invoke('set_click_through', { ignore });

export const getSettings = (): Promise<Settings> => invoke('get_settings');
export const saveSettings = (settings: Settings): Promise<void> =>
  invoke('save_settings', { settings });
