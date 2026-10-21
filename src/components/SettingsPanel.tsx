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
import type { Settings } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';

export function SettingsPanel(): React.JSX.Element {
  const opacity = useAppStore((s) => s.ui.opacity);
  const setOpacity = useAppStore((s) => s.setOpacity);
  const setBuild = useAppStore((s) => s.setBuild);
  const hasBuild = useAppStore((s) => s.build !== null);
  const mode = useAppStore((s) => s.source.mode);

  const textScale = useAppStore((s) => s.ui.textScale);
  const setTextScale = useAppStore((s) => s.setTextScale);

  // Read-modify-write: the backend saves the route position into the same file.
  const save = async (patch: Partial<Settings>): Promise<void> => {
    const current = await getSettings();
    await saveSettings({ ...current, ...patch });
  };
  const onOpacityChange = (value: number): void => {
    setOpacity(value);
    void save({ opacity: value });
  };
  const onTextScaleChange = (value: number): void => {
    setTextScale(value);
    void save({ textScale: value });
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
      <label className="flex items-center gap-2">
        <span className="w-20 font-caps text-label">Text size</span>
        <input
          type="range"
          min={0.8}
          max={1.5}
          step={0.05}
          value={textScale}
          onChange={(e) => onTextScaleChange(Number(e.target.value))}
          className="flex-1"
        />
        <span className="w-10 text-right text-value">{Math.round(textScale * 100)}%</span>
      </label>
      <div className="flex items-center gap-3">
        <label className="flex flex-1 items-center gap-2">
          <span className="w-20 font-caps text-label">Opacity</span>
          <input
            type="range"
            min={0.2}
            max={1}
            step={0.05}
            value={opacity}
            onChange={(e) => onOpacityChange(Number(e.target.value))}
            className="flex-1"
          />
        </label>
        <button className="btn" onClick={() => void snapOverlayBottomCenter()}>
          Move to bottom center
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="w-20 font-caps text-label">Log</span>
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
        <span className="w-20 font-caps text-label">Build</span>
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
