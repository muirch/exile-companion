import { useEffect } from 'react';

import { OverlayRoot } from './components/OverlayRoot';
import { getState } from './ipc/commands';
import { isTauri } from './ipc/env';
import {
  onCampaignState,
  onOverlayGrab,
  onReplayProgress,
  onSourceStatus,
} from './ipc/events';
import { startMock, stopMock } from './ipc/mock';
import { useAppStore } from './store/useAppStore';

export function App(): React.JSX.Element {
  const setCampaign = useAppStore((s) => s.setCampaign);
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
      onSourceStatus(setSourceStatus),
      onReplayProgress(setReplayProgress),
      onOverlayGrab(setGrabbed),
    ]);

    // Hydrate the current state for a late-mounting overlay window.
    getState()
      .then(setCampaign)
      .catch(() => {
        /* backend not ready / no state yet - the push listener will fill it in */
      });

    return () => {
      void unlisteners.then((list) => list.forEach((unlisten) => unlisten()));
    };
  }, [setCampaign, setSourceStatus, setReplayProgress, setGrabbed]);

  return <OverlayRoot />;
}
