import { useEffect } from 'react';

import {
  getBuild,
  getGuidance,
  getHotkeys,
  getOverlayState,
  getRoute,
  getRouteEnabled,
  getSettings,
  getSource,
  getState,
} from './ipc/commands';
import { isTauri } from './ipc/env';
import {
  onBuildUpdated,
  onCampaignState,
  onGameFocus,
  onGuidance,
  onHotkeysChanged,
  onOverlayGrab,
  onReplayProgress,
  onRouteEnabled,
  onRouteView,
  onSettingsChanged,
  onSourceStatus,
} from './ipc/events';
import { startMock, stopMock } from './ipc/mock';
import type { Settings } from './ipc/types';
import { useAppStore } from './store/useAppStore';

function applySettings(s: Settings): void {
  useAppStore.getState().setOpacity(s.opacity);
  useAppStore.getState().setTextScale(s.textScale);
}

/** Each window is its own web page, so each one subscribes and hydrates by itself. */
export function useBackendSync(): void {
  useEffect(() => {
    // In a plain browser (pnpm dev) there is no backend: drive the UI from the mock instead.
    if (!isTauri()) {
      startMock();
      return () => stopMock();
    }

    const st = useAppStore.getState();
    const unlisteners = Promise.all([
      onCampaignState(st.setCampaign),
      onBuildUpdated(st.setBuild),
      onGuidance(st.setGuidance),
      onRouteView(st.setRoute),
      onRouteEnabled(st.setRouteEnabled),
      onSourceStatus(st.setSourceStatus),
      onReplayProgress(st.setReplayProgress),
      onOverlayGrab(st.setGrabbed),
      onGameFocus(st.setGameFocused),
      onSettingsChanged(applySettings),
      onHotkeysChanged((v) => st.setHotkeys(v.hotkeys)),
    ]);

    // Read the current state only once the listeners are in: whatever the backend pushes from
    // here on is heard, and whatever it pushed before is in the reply.
    void unlisteners.then(() => {
      getState().then(st.setCampaign).catch(noop);
      getBuild().then(st.setBuild).catch(noop);
      getGuidance().then(st.setGuidance).catch(noop);
      getRoute().then(st.setRoute).catch(noop);
      getRouteEnabled().then(st.setRouteEnabled).catch(noop);
      getSource().then(st.setSourceStatus).catch(noop);
      getSettings().then(applySettings).catch(noop);
      getHotkeys()
        .then((v) => st.setHotkeys(v.hotkeys))
        .catch(noop);
      getOverlayState()
        .then((o) => {
          st.setGrabbed(o.grabbed);
          st.setGameFocused(o.gameFocused);
        })
        .catch(noop);
    });

    return () => {
      void unlisteners.then((list) => list.forEach((unlisten) => unlisten()));
    };
  }, []);
}

/** Backend not ready / no state yet - the push listeners will fill things in. */
function noop(): void {}
