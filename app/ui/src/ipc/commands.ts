// Typed wrappers over the Tauri command surface (`exile_companion::commands`).
//
// In a plain browser (`pnpm dev`, no Tauri) these fall back to the in-browser mock so the UI is
// fully explorable without a backend.

import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { readText } from '@tauri-apps/plugin-clipboard-manager';

import { isTauri } from './env';
import * as mock from './mock';
import { useAppStore } from '../store/useAppStore';
import type {
  AgentId,
  AssistantView,
  BuildSummary,
  CampaignState,
  BindError,
  ChatProvider,
  ChatView,
  HotkeySettings,
  NextCard,
  NextObjective,
  PobInfo,
  PobSync,
  RouteView,
  Settings,
  SourceStatus,
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
  area_id: null,
};

export const pickLogFile = (): Promise<string | null> =>
  isTauri() ? invoke('pick_log_file') : Promise.resolve('docs/sample-client.txt');

const MOCK_POB: PobInfo = {
  path: 'C:\\Users\\exile\\AppData\\Roaming\\Path of Building Community',
  version: '2.67.2',
};

export const getPob = (): Promise<PobInfo | null> =>
  isTauri() ? invoke('get_pob') : Promise.resolve(MOCK_POB);

/** Null when the dialog is cancelled. Rejects with a message when the folder isn't PoB. */
export const pickPobFolder = (): Promise<PobInfo | null> =>
  isTauri() ? invoke('pick_pob_folder') : Promise.resolve(MOCK_POB);

const MOCK_BUILDS = 'C:\\Users\\exile\\Documents\\Path of Building\\Builds\\';
let mockSync: PobSync = { enabled: false, file: `${MOCK_BUILDS}Exile Companion.xml`, picked: false };

export const getPobSync = (): Promise<PobSync> =>
  isTauri() ? invoke('get_pob_sync') : Promise.resolve(mockSync);

export const setPobSync = (enabled: boolean): Promise<PobSync> =>
  isTauri() ? invoke('set_pob_sync', { enabled }) : Promise.resolve((mockSync = { ...mockSync, enabled }));

/** Null when the dialog is cancelled. The picked build replaces the app's build. */
export const pickPobBuild = (): Promise<PobSync | null> =>
  isTauri()
    ? invoke('pick_pob_build')
    : Promise.resolve((mockSync = { enabled: true, file: `${MOCK_BUILDS}Pathfinder PConc.xml`, picked: true }));

export const unpickPobBuild = (): Promise<PobSync> =>
  isTauri()
    ? invoke('unpick_pob_build')
    : Promise.resolve((mockSync = { ...mockSync, file: `${MOCK_BUILDS}Exile Companion.xml`, picked: false }));

export const openPob = (): Promise<void> => (isTauri() ? invoke('open_pob') : Promise.resolve());

const MOCK_ASSISTANT: AssistantView = {
  server: 'C:\\Users\\exile\\AppData\\Local\\Exile Companion\\exile-companion-mcp.exe',
  agents: [
    { id: 'claudeDesktop', name: 'Claude Desktop', status: { state: 'connected' }, config: '' },
    { id: 'claudeCode', name: 'Claude Code', status: { state: 'notConnected' }, config: '' },
    { id: 'cursor', name: 'Cursor', status: { state: 'notFound' }, config: '' },
    { id: 'vsCode', name: 'VS Code (Copilot)', status: { state: 'notFound' }, config: '' },
    { id: 'codex', name: 'Codex', status: { state: 'otherCopy', command: 'D:\\old\\exile-companion-mcp.exe' }, config: '' },
  ],
};

export const getAssistant = (): Promise<AssistantView> =>
  isTauri() ? invoke('get_assistant') : Promise.resolve(MOCK_ASSISTANT);

/** Rejects with a message when the agent's config couldn't be changed. */
export const connectAgent = (id: AgentId): Promise<AssistantView> => {
  if (isTauri()) return invoke('connect_agent', { id });
  const agents = MOCK_ASSISTANT.agents.map((a) =>
    a.id === id ? { ...a, status: { state: 'connected' as const } } : a,
  );
  return Promise.resolve({ ...MOCK_ASSISTANT, agents });
};

export const copyAssistantConfig = (): Promise<void> =>
  isTauri() ? invoke('copy_assistant_config') : Promise.resolve();

const MOCK_HAS_KEY = mock.chatHasKeyMock();
let mockChat: ChatView = {
  keys: { anthropic: MOCK_HAS_KEY, openRouter: false, custom: false },
  available: true,
  busy: false,
  missing: MOCK_HAS_KEY ? null : 'Add your Anthropic API key in the settings.',
  model: 'Sonnet 5.5',
};

export const getChat = (): Promise<ChatView> =>
  isTauri() ? invoke('get_chat') : Promise.resolve(mockChat);

/** Rejects with a message when the system's credential store refused the key. */
export const setApiKey = (provider: ChatProvider, key: string): Promise<ChatView> => {
  if (isTauri()) return invoke('set_api_key', { provider, key });
  mockChat = { ...mockChat, keys: { ...mockChat.keys, [provider]: true }, missing: null };
  return Promise.resolve(mockChat);
};

export const forgetApiKey = (provider: ChatProvider): Promise<ChatView> => {
  if (isTauri()) return invoke('forget_api_key', { provider });
  mockChat = { ...mockChat, keys: { ...mockChat.keys, [provider]: false } };
  return Promise.resolve(mockChat);
};

/** Runs the tools without a model, so it works without a key and costs nothing. */
export const getNextCard = (): Promise<NextCard> =>
  isTauri() ? invoke('get_next_card') : Promise.resolve(mock.nextCardMock());

/** The answer comes as chat://event and ends with chat://over. Rejects while one is running. */
export const chatSend = (question: string): Promise<void> =>
  isTauri() ? invoke('chat_send', { question }) : mock.chatMock(question);

export const chatStop = (): Promise<void> =>
  isTauri() ? invoke('chat_stop') : Promise.resolve(mock.chatStopMock());

export const chatNew = (): Promise<void> =>
  isTauri() ? invoke('chat_new') : Promise.resolve(mock.chatStopMock());

export const openChat = (): Promise<void> => (isTauri() ? invoke('open_chat') : Promise.resolve());

export const closeChat = (): Promise<void> => (isTauri() ? invoke('close_chat') : Promise.resolve());

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
  isTauri()
    ? invoke('get_settings')
    : Promise.resolve({
        opacity: 0.8,
        lastLogPath: null,
        routeEdge: 0,
        leagueStart: true,
        textScale: 1,
        gemHints: 'rewardsAndVendors',
        compact: useAppStore.getState().ui.compact,
        fadeAfter: useAppStore.getState().ui.fadeAfter,
        presetStages: [],
        pobPath: null,
        pobSync: false,
        pobBuild: null,
        chat: {
          provider: 'anthropic',
          model: 'claude-sonnet-5-5',
          effort: 'low',
          routerModel: '',
          customUrl: '',
          customModel: '',
        },
      });

export const saveSettings = (settings: Settings): Promise<void> =>
  isTauri() ? invoke('save_settings', { settings }) : Promise.resolve();

// Read-modify-write: the backend saves the route position and anchor into the same file. Without
// a backend the store stands in for the change event.
export async function updateSettings(patch: Partial<Settings>): Promise<void> {
  if (!isTauri()) {
    const { ui, setCompact, setFadeAfter } = useAppStore.getState();
    setCompact(patch.compact ?? ui.compact);
    setFadeAfter(patch.fadeAfter ?? ui.fadeAfter);
    return;
  }
  const current = await getSettings();
  await saveSettings({ ...current, ...patch });
}

export const getRoute = (): Promise<RouteView | null> =>
  isTauri() ? invoke('get_route') : Promise.resolve(mock.routeMock());

export const routeNext = (): Promise<RouteView | null> =>
  isTauri() ? invoke('route_next') : Promise.resolve(mock.moveRouteMock(1));

export const routePrev = (): Promise<RouteView | null> =>
  isTauri() ? invoke('route_prev') : Promise.resolve(mock.moveRouteMock(-1));

export const routeReset = (): Promise<RouteView | null> =>
  isTauri() ? invoke('route_reset') : Promise.resolve(mock.moveRouteMock(-Infinity));

export const getSource = (): Promise<SourceStatus> =>
  isTauri() ? invoke('get_source') : Promise.resolve({ mode: 'replay', path: 'mock' });

export interface OverlayState {
  grabbed: boolean;
  gameFocused: boolean;
}

export const getOverlayState = (): Promise<OverlayState> =>
  isTauri() ? invoke('get_overlay_state') : Promise.resolve({ grabbed: true, gameFocused: true });

export const snapOverlayBottomCenter = (): Promise<void> =>
  isTauri() ? invoke('snap_overlay_bottom_center') : Promise.resolve();

export const fitOverlay = (width: number, height: number): Promise<void> =>
  isTauri() ? invoke('fit_overlay', { width, height }) : Promise.resolve();

export const openSettings = (): Promise<void> =>
  isTauri() ? invoke('open_settings') : Promise.resolve();

export const startMove = (): Promise<void> =>
  isTauri() ? invoke('start_move') : Promise.resolve();

export const lockOverlay = (): Promise<void> =>
  isTauri() ? invoke('lock_overlay') : Promise.resolve(useAppStore.getState().setGrabbed(false));

export const closeMenu = (): Promise<void> =>
  isTauri() ? invoke('close_menu') : Promise.resolve();

export const closeSettings = (): Promise<void> =>
  isTauri() ? getCurrentWindow().hide() : Promise.resolve();

export const quitApp = (): Promise<void> => (isTauri() ? invoke('quit_app') : Promise.resolve());

export const getRouteEnabled = (): Promise<boolean> =>
  isTauri() ? invoke('get_route_enabled') : Promise.resolve(true);

export const setRouteEnabled = (enabled: boolean): Promise<void> =>
  isTauri()
    ? invoke('set_route_enabled', { enabled })
    : Promise.resolve(useAppStore.getState().setRouteEnabled(enabled));

const DEFAULT_HOTKEYS: HotkeySettings = {
  hotkeys: {
    menuHold: 'Backquote',
    moveOverlay: 'Alt+KeyM',
    routePrev: 'Alt+ArrowLeft',
    routeNext: 'Alt+ArrowRight',
    relog: 'F9',
    openChat: 'Alt+KeyA',
    vendorSearch: 'Alt+KeyG',
  },
  relogEnabled: false,
  vendorSearchEnabled: false,
};
let mockHotkeys = DEFAULT_HOTKEYS;

export const getHotkeys = (): Promise<HotkeySettings> =>
  isTauri() ? invoke('get_hotkeys') : Promise.resolve(mockHotkeys);

export const setHotkeys = (value: HotkeySettings): Promise<BindError[]> => {
  if (isTauri()) return invoke('set_hotkeys', { value });
  mockHotkeys = value;
  return Promise.resolve([]);
};

export const pauseHotkeys = (): Promise<void> =>
  isTauri() ? invoke('pause_hotkeys') : Promise.resolve();

export const resumeHotkeys = (): Promise<BindError[]> =>
  isTauri() ? invoke('resume_hotkeys') : Promise.resolve([]);
