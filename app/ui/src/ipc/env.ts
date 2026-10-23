/**
 * True when the UI is running inside the Tauri webview, false in a plain browser (`pnpm dev`).
 *
 * In browser mode the IPC layer swaps in a mock (see `mock.ts`) so the overlay can be developed and
 * styled on Linux without Tauri, webkit, or Windows.
 */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
