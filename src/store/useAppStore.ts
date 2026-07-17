import { create } from 'zustand';

import type { CampaignState, ReplayProgress, SourceMode, SourceStatus } from '../ipc/types';

const EMPTY_CAMPAIGN: CampaignState = {
  character: null,
  class: null,
  level: 0,
  act: null,
  zone: null,
  location_kind: 'unknown',
  deaths: 0,
  session_started: false,
};

interface AppStore {
  campaign: CampaignState;
  source: { mode: SourceMode; path: string | null; error: string | null };
  replay: ReplayProgress;
  ui: { grabbed: boolean; opacity: number };

  setCampaign: (state: CampaignState) => void;
  setSourceStatus: (status: SourceStatus) => void;
  setReplayProgress: (progress: ReplayProgress) => void;
  setGrabbed: (grabbed: boolean) => void;
  setOpacity: (opacity: number) => void;
}

/** Single source of truth for push-based state; IPC listeners are its only writers. */
export const useAppStore = create<AppStore>((set) => ({
  campaign: EMPTY_CAMPAIGN,
  source: { mode: 'idle', path: null, error: null },
  replay: { position: 0, total: 0, speed: 'X1', paused: false },
  ui: { grabbed: true, opacity: 0.85 },

  setCampaign: (campaign) => set({ campaign }),
  setSourceStatus: (status) =>
    set({ source: { mode: status.mode, path: status.path ?? null, error: status.error ?? null } }),
  setReplayProgress: (replay) => set({ replay }),
  setGrabbed: (grabbed) => set((s) => ({ ui: { ...s.ui, grabbed } })),
  setOpacity: (opacity) => set((s) => ({ ui: { ...s.ui, opacity } })),
}));
