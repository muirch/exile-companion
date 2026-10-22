import { create } from 'zustand';

import type {
  BuildSummary,
  Hotkeys,
  CampaignState,
  NextObjective,
  ReplayProgress,
  RouteView,
  SourceMode,
  SourceStatus,
} from '../ipc/types';

const EMPTY_CAMPAIGN: CampaignState = {
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

interface AppStore {
  campaign: CampaignState;
  build: BuildSummary | null;
  guidance: NextObjective | null;
  route: RouteView | null;
  routeEnabled: boolean;
  hotkeys: Hotkeys;
  source: { mode: SourceMode; path: string | null; error: string | null };
  replay: ReplayProgress;
  ui: {
    grabbed: boolean;
    opacity: number;
    gameFocused: boolean;
    textScale: number;
    compact: boolean;
    fadeAfter: number;
    /** Counts menu-key taps; each one shows a faded overlay again. */
    peeks: number;
  };

  setCampaign: (state: CampaignState) => void;
  setBuild: (build: BuildSummary | null) => void;
  setGuidance: (guidance: NextObjective | null) => void;
  setRoute: (route: RouteView | null) => void;
  setRouteEnabled: (routeEnabled: boolean) => void;
  setHotkeys: (hotkeys: Hotkeys) => void;
  setSourceStatus: (status: SourceStatus) => void;
  setReplayProgress: (progress: ReplayProgress) => void;
  setGrabbed: (grabbed: boolean) => void;
  setGameFocused: (gameFocused: boolean) => void;
  setOpacity: (opacity: number) => void;
  setTextScale: (textScale: number) => void;
  setCompact: (compact: boolean) => void;
  setFadeAfter: (fadeAfter: number) => void;
  peek: () => void;
}

/** Single source of truth for push-based state; IPC listeners are its only writers. */
export const useAppStore = create<AppStore>((set) => ({
  campaign: EMPTY_CAMPAIGN,
  build: null,
  guidance: null,
  route: null,
  routeEnabled: true,
  hotkeys: {
    menuHold: 'Backquote',
    moveOverlay: 'Alt+KeyM',
    routePrev: 'Alt+ArrowLeft',
    routeNext: 'Alt+ArrowRight',
    relog: 'F9',
  },
  source: { mode: 'idle', path: null, error: null },
  replay: { position: 0, total: 0, speed: 'X1', paused: false },
  // Hidden and locked until the backend (or the browser mock) says otherwise, so the first frame
  // never flashes the full overlay over the desktop.
  ui: {
    grabbed: false,
    opacity: 0.8,
    gameFocused: false,
    textScale: 1,
    compact: true,
    fadeAfter: 0,
    peeks: 0,
  },

  setCampaign: (campaign) => set({ campaign }),
  setBuild: (build) => set({ build }),
  setGuidance: (guidance) => set({ guidance }),
  setRoute: (route) => set({ route }),
  setRouteEnabled: (routeEnabled) => set({ routeEnabled }),
  setHotkeys: (hotkeys) => set({ hotkeys }),
  setSourceStatus: (status) =>
    set({ source: { mode: status.mode, path: status.path ?? null, error: status.error ?? null } }),
  setReplayProgress: (replay) => set({ replay }),
  setGrabbed: (grabbed) => set((s) => ({ ui: { ...s.ui, grabbed } })),
  setGameFocused: (gameFocused) => set((s) => ({ ui: { ...s.ui, gameFocused } })),
  setOpacity: (opacity) => set((s) => ({ ui: { ...s.ui, opacity } })),
  setTextScale: (textScale) => set((s) => ({ ui: { ...s.ui, textScale } })),
  setCompact: (compact) => set((s) => ({ ui: { ...s.ui, compact } })),
  setFadeAfter: (fadeAfter) => set((s) => ({ ui: { ...s.ui, fadeAfter } })),
  peek: () => set((s) => ({ ui: { ...s.ui, peeks: s.ui.peeks + 1 } })),
}));
