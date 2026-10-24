// Typed listeners for backend -> frontend events (`exile_companion` emits these).

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { isTauri } from './env';
import { onChatMock } from './mock';

import type {
  BuildSummary,
  CampaignState,
  ChatEvent,
  TurnOver,
  NextObjective,
  ReplayProgress,
  HotkeySettings,
  RouteView,
  Settings,
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

/** A tap of the menu key in the game: shows a faded overlay again. */
export const onOverlayPeek = (cb: () => void): Promise<UnlistenFn> =>
  listen('overlay://peek', () => cb());

export const onSettingsChanged = (cb: (settings: Settings) => void): Promise<UnlistenFn> =>
  listen<Settings>('settings://changed', (e) => cb(e.payload));

export const onRouteEnabled = (cb: (enabled: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>('features://route', (e) => cb(e.payload));

export const onMenuOpened = (cb: () => void): Promise<UnlistenFn> =>
  listen('menu://opened', () => cb());

export const onHotkeysChanged = (cb: (value: HotkeySettings) => void): Promise<UnlistenFn> =>
  listen<HotkeySettings>('hotkeys://changed', (e) => cb(e.payload));

// In the browser the mock answers instead of the backend.
export const onChatEvent = (cb: (event: ChatEvent) => void): Promise<UnlistenFn> =>
  isTauri()
    ? listen<ChatEvent>('chat://event', (e) => cb(e.payload))
    : Promise.resolve(onChatMock({ event: cb }));

export const onChatOver = (cb: (over: TurnOver) => void): Promise<UnlistenFn> =>
  isTauri()
    ? listen<TurnOver>('chat://over', (e) => cb(e.payload))
    : Promise.resolve(onChatMock({ over: cb }));

export const onChatOpened = (cb: () => void): Promise<UnlistenFn> =>
  listen('chat://opened', () => cb());
