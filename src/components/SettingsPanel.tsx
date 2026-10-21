import {
  clearBuild,
  getSettings,
  importBuildFile,
  pasteBuild,
  pickLogFile,
  saveSettings,
  snapOverlayBottomCenter,
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
    <div className="flex flex-col gap-2.5 text-[13px]">
      <div className="flex items-center gap-3">
        <label className="flex flex-1 items-center gap-2">
          <span className="font-caps text-ash-dim">Opacity</span>
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
        <button className="btn" onClick={() => void snapOverlayBottomCenter()}>
          Move to bottom center
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="w-12 font-caps text-ash-dim">Log</span>
        <button className="btn" onClick={watchLive}>
          Pick Client.txt
        </button>
        <button className="btn" onClick={replayLog}>
          Replay a log
        </button>
        {mode !== 'idle' && (
          <button className="btn" onClick={stopSource}>
            Stop
          </button>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="w-12 font-caps text-ash-dim">Build</span>
        <button className="btn" onClick={importFromFile}>
          Import PoB file
        </button>
        <button className="btn" onClick={importFromClipboard}>
          Paste PoB code
        </button>
        {hasBuild && (
          <button className="btn" onClick={() => void clearBuild()}>
            Clear build
          </button>
        )}
      </div>
    </div>
  );
}
