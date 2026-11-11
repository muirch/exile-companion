import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { App } from './App';
import { isTauri } from './ipc/env';
import './styles.css';

// The fonts are font-display: block, so text that shows up late (the tree's, after its data)
// stays hidden while its font loads. Loading them all at once keeps screenshots of the browser
// mock whole. The app reads them from its own files, so it doesn't need this.
if (!isTauri()) {
  for (const font of ['400 13px "Alegreya Sans"', '500 13px "Alegreya Sans"', 'italic 400 13px "Alegreya Sans"', '500 13px "Alegreya Sans SC"']) {
    void document.fonts.load(font);
  }
}

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
});

const rootEl = document.getElementById('root');
if (!rootEl) throw new Error('missing #root element');

ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
