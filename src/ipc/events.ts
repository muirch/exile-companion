// Typed listeners for backend -> frontend events (`poebo_app` emits these).

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { CampaignState, ReplayProgress, SourceStatus } from './types';

export const CAMPAIGN_STATE = 'campaign://state-updated';
export const SOURCE_STATUS = 'source://status';
export const REPLAY_PROGRESS = 'replay://progress';
export const OVERLAY_GRAB = 'overlay://grab-changed';

export const onCampaignState = (cb: (state: CampaignState) => void): Promise<UnlistenFn> =>
  listen<CampaignState>(CAMPAIGN_STATE, (e) => cb(e.payload));

export const onSourceStatus = (cb: (status: SourceStatus) => void): Promise<UnlistenFn> =>
  listen<SourceStatus>(SOURCE_STATUS, (e) => cb(e.payload));

export const onReplayProgress = (cb: (progress: ReplayProgress) => void): Promise<UnlistenFn> =>
  listen<ReplayProgress>(REPLAY_PROGRESS, (e) => cb(e.payload));

export const onOverlayGrab = (cb: (grabbed: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>(OVERLAY_GRAB, (e) => cb(e.payload));
