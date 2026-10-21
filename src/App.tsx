import { useEffect } from 'react';

import { OverlayRoot } from './components/OverlayRoot';
import { getBuild, getGuidance, getRoute, getSource, getState } from './ipc/commands';
import { isTauri } from './ipc/env';
import {
  onBuildUpdated,
  onCampaignState,
  onGuidance,
  onOverlayGrab,
  onReplayProgress,
  onRouteView,
  onSourceStatus,
} from './ipc/events';
import { startMock, stopMock } from './ipc/mock';
import { useAppStore } from './store/useAppStore';

export function App(): React.JSX.Element {
  const setCampaign = useAppStore((s) => s.setCampaign);
  const setBuild = useAppStore((s) => s.setBuild);
  const setGuidance = useAppStore((s) => s.setGuidance);
  const setRoute = useAppStore((s) => s.setRoute);
  const setSourceStatus = useAppStore((s) => s.setSourceStatus);
  const setReplayProgress = useAppStore((s) => s.setReplayProgress);
  const setGrabbed = useAppStore((s) => s.setGrabbed);

  useEffect(() => {
    // In a plain browser (pnpm dev) there is no backend: drive the UI from the mock instead.
    if (!isTauri()) {
      startMock();
      return () => stopMock();
    }

    // Register all push listeners once.
    const unlisteners = Promise.all([
      onCampaignState(setCampaign),
      onBuildUpdated(setBuild),
      onGuidance(setGuidance),
      onRouteView(setRoute),
      onSourceStatus(setSourceStatus),
      onReplayProgress(setReplayProgress),
      onOverlayGrab(setGrabbed),
    ]);

    // Hydrate current state / build / objective for a late-mounting overlay window.
    getState().then(setCampaign).catch(noop);
    getBuild().then(setBuild).catch(noop);
    getGuidance().then(setGuidance).catch(noop);
    getRoute().then(setRoute).catch(noop);
    getSource().then(setSourceStatus).catch(noop);

    return () => {
      void unlisteners.then((list) => list.forEach((unlisten) => unlisten()));
    };
  }, [setCampaign, setBuild, setGuidance, setRoute, setSourceStatus, setReplayProgress, setGrabbed]);

  return <OverlayRoot />;
}

/** Backend not ready / no state yet - the push listeners will fill things in. */
function noop(): void {}
