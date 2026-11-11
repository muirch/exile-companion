import { getCurrentWindow } from '@tauri-apps/api/window';

import { ChatApp } from './components/ChatApp';
import { GemsApp } from './components/GemsApp';
import { MenuApp } from './components/MenuApp';
import { OverlayRoot } from './components/OverlayRoot';
import { SettingsApp } from './components/SettingsApp';
import { TreeApp } from './components/TreeApp';
import { isTauri } from './ipc/env';
import { useBackendSync } from './useBackendSync';

// One bundle, six windows (see tauri.conf.json). The browser mock picks one with ?window=.
function windowLabel(): string {
  if (isTauri()) return getCurrentWindow().label;
  return new URLSearchParams(window.location.search).get('window') ?? 'overlay';
}

const LABEL = windowLabel();
document.documentElement.dataset.window = LABEL;

export function App(): React.JSX.Element {
  useBackendSync();
  if (LABEL === 'menu') return <MenuApp />;
  if (LABEL === 'settings') return <SettingsApp />;
  if (LABEL === 'chat') return <ChatApp />;
  if (LABEL === 'gems') return <GemsApp />;
  if (LABEL === 'tree') return <TreeApp />;
  return <OverlayRoot />;
}
