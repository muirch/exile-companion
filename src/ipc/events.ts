// Typed listeners for backend -> frontend events (`poebo_app` emits these).

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type {
  BuildSummary,
  CampaignState,
  NextObjective,
  ReplayProgress,
  RouteView,
  SourceStatus,
} from './types';

export const CAMPAIGN_STATE = 'campaign://state-updated';
export const SOURCE_STATUS = 'source://status';
export const REPLAY_PROGRESS = 'replay://progress';
export const OVERLAY_GRAB = 'overlay://grab-changed';
export const BUILD_UPDATED = 'build://updated';
export const GUIDANCE_NEXT = 'guidance://next';
export const ROUTE_VIEW = 'route://view';
export const GAME_FOCUS = 'overlay://game-focus';

export const onCampaignState = (cb: (state: CampaignState) => void): Promise<UnlistenFn> =>
  listen<CampaignState>(CAMPAIGN_STATE, (e) => cb(e.payload));

export const onSourceStatus = (cb: (status: SourceStatus) => void): Promise<UnlistenFn> =>
  listen<SourceStatus>(SOURCE_STATUS, (e) => cb(e.payload));

export const onReplayProgress = (cb: (progress: ReplayProgress) => void): Promise<UnlistenFn> =>
  listen<ReplayProgress>(REPLAY_PROGRESS, (e) => cb(e.payload));

export const onOverlayGrab = (cb: (grabbed: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>(OVERLAY_GRAB, (e) => cb(e.payload));

export const onBuildUpdated = (cb: (build: BuildSummary | null) => void): Promise<UnlistenFn> =>
  listen<BuildSummary | null>(BUILD_UPDATED, (e) => cb(e.payload));

export const onGuidance = (cb: (objective: NextObjective) => void): Promise<UnlistenFn> =>
  listen<NextObjective>(GUIDANCE_NEXT, (e) => cb(e.payload));

export const onRouteView = (cb: (view: RouteView | null) => void): Promise<UnlistenFn> =>
  listen<RouteView | null>(ROUTE_VIEW, (e) => cb(e.payload));

export const onGameFocus = (cb: (focused: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>(GAME_FOCUS, (e) => cb(e.payload));
