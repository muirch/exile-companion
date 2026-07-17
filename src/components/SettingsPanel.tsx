import { pickLogFile, saveSettings, startLive, startReplay, stopSource } from '../ipc/commands';
import { useAppStore } from '../store/useAppStore';

export function SettingsPanel(): React.JSX.Element {
  const opacity = useAppStore((s) => s.ui.opacity);
  const setOpacity = useAppStore((s) => s.setOpacity);
  const mode = useAppStore((s) => s.source.mode);
  const path = useAppStore((s) => s.source.path);

  const onOpacityChange = (value: number): void => {
    setOpacity(value);
    void saveSettings({ opacity: value, lastLogPath: path });
  };

  const watchLive = async (): Promise<void> => {
    const picked = await pickLogFile();
    if (picked) await startLive(picked);
  };

  const replayLog = async (): Promise<void> => {
    const picked = await pickLogFile();
    if (picked) await startReplay(picked, 'X5');
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
          onChange={(e) => onOpacityChange(Number(e.target.value))}
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
          Replay log…
        </button>
        {mode !== 'idle' && (
          <button className="rounded bg-slate-700 px-2 py-0.5 hover:bg-slate-600" onClick={stopSource}>
            Stop
          </button>
        )}
      </div>
    </div>
  );
}
