import {
  clearBuild,
  getSettings,
  importBuildFile,
  pasteBuild,
  pickLogFile,
  saveSettings,
  startLive,
  startReplay,
  stopSource,
} from '../ipc/commands';
import { useAppStore } from '../store/useAppStore';

export function SettingsPanel(): React.JSX.Element {
  const opacity = useAppStore((s) => s.ui.opacity);
  const setOpacity = useAppStore((s) => s.setOpacity);
  const setBuild = useAppStore((s) => s.setBuild);
  const hasBuild = useAppStore((s) => s.build !== null);
  const mode = useAppStore((s) => s.source.mode);
  const path = useAppStore((s) => s.source.path);

  // Read-modify-write: the backend saves the route position into the same file.
  const onOpacityChange = async (value: number): Promise<void> => {
    setOpacity(value);
    const current = await getSettings();
    await saveSettings({ ...current, opacity: value, lastLogPath: path });
  };

  const watchLive = async (): Promise<void> => {
    const picked = await pickLogFile();
    if (picked) await startLive(picked);
  };

  const replayLog = async (): Promise<void> => {
    const picked = await pickLogFile();
    if (picked) await startReplay(picked, 'X5');
  };

  // The backend also emits build://updated; setting from the return value is just immediate feedback.
  const importFromFile = async (): Promise<void> => {
    const summary = await importBuildFile();
    if (summary) setBuild(summary);
  };

  const importFromClipboard = async (): Promise<void> => {
    try {
      setBuild(await pasteBuild());
    } catch {
      /* nothing valid on the clipboard - leave the current build in place */
    }
  };

  return (
    <div className="mt-2 flex flex-col gap-2 border-t border-slate-700/50 pt-2 text-xs">
      <label className="flex items-center gap-2">
        <span className="text-slate-400">Opacity</span>
        <input
          type="range"
          min={0.2}
          max={1}
          step={0.05}
          value={opacity}
          onChange={(e) => void onOpacityChange(Number(e.target.value))}
          className="flex-1"
        />
      </label>
      <div className="flex flex-wrap gap-1">
        <button
          className="rounded bg-emerald-700 px-2 py-0.5 hover:bg-emerald-600"
          onClick={watchLive}
        >
          Watch Client.txt
        </button>
        <button
          className="rounded bg-indigo-700 px-2 py-0.5 hover:bg-indigo-600"
          onClick={replayLog}
        >
          Replay log...
        </button>
        {mode !== 'idle' && (
          <button className="rounded bg-slate-700 px-2 py-0.5 hover:bg-slate-600" onClick={stopSource}>
            Stop
          </button>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-1">
        <span className="mr-1 text-slate-400">Build</span>
        <button
          className="rounded bg-fuchsia-800 px-2 py-0.5 hover:bg-fuchsia-700"
          onClick={importFromFile}
        >
          Import file...
        </button>
        <button
          className="rounded bg-fuchsia-800 px-2 py-0.5 hover:bg-fuchsia-700"
          onClick={importFromClipboard}
        >
          Paste
        </button>
        {hasBuild && (
          <button
            className="rounded bg-slate-700 px-2 py-0.5 hover:bg-slate-600"
            onClick={() => void clearBuild()}
          >
            Clear
          </button>
        )}
      </div>
    </div>
  );
}
