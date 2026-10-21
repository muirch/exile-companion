import { routeNext, routePrev } from '../ipc/commands';
import type { Part, Step } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';

// 0 is north, clockwise in 45 degree steps (exile-leveling's convention).
const ARROWS = ['↑', '↗', '→', '↘', '↓', '↙', '←', '↖'];

function PartView({ part }: { part: Part }): React.JSX.Element {
  switch (part.kind) {
    case 'text':
      return <span>{part.text}</span>;
    case 'kill':
      return <span className="text-rose-300">{part.name}</span>;
    case 'arena':
      return <span className="text-amber-300">{part.name}</span>;
    case 'area':
    case 'enter':
      return <span className="text-cyan-300">{part.name}</span>;
    case 'logout':
      return (
        <span>
          Logout to <span className="text-cyan-300">{part.town}</span>
        </span>
      );
    case 'waypoint':
    case 'waypoint_get':
      return <span className="text-sky-300">waypoint</span>;
    case 'waypoint_use':
      return (
        <span>
          <span className="text-sky-300">Waypoint</span> to{' '}
          <span className="text-cyan-300">{part.name}</span>
        </span>
      );
    case 'portal_set':
      return <span className="text-sky-300">portal</span>;
    case 'portal_use':
      return (
        <span>
          <span className="text-sky-300">portal</span> to{' '}
          <span className="text-cyan-300">{part.name}</span>
        </span>
      );
    case 'quest':
      return <span className="text-yellow-300">{part.name}</span>;
    case 'quest_text':
    case 'generic':
      return <span className="text-yellow-200/90">{part.text}</span>;
    case 'reward_quest':
    case 'reward_vendor':
      return <span className="text-emerald-300">{part.item}</span>;
    case 'trial':
      return <span className="text-amber-300">Trial of Ascendancy</span>;
    case 'ascend':
      return <span className="text-amber-300">Ascend ({part.difficulty} lab)</span>;
    case 'crafting':
      return <span className="text-violet-300">crafting recipe</span>;
    case 'dir':
      return <span className="font-bold text-cyan-200">{ARROWS[part.degrees / 45]}</span>;
    case 'copy':
      return <span className="font-mono text-slate-300">{part.text}</span>;
  }
}

function Parts({ parts }: { parts: Part[] }): React.JSX.Element {
  return (
    <>
      {parts.map((p, i) => (
        <PartView key={i} part={p} />
      ))}
    </>
  );
}

function StepView({ step }: { step: Step }): React.JSX.Element {
  return (
    <li className="leading-snug">
      <Parts parts={step.parts} />
      {step.sub_steps.length > 0 && (
        <ul className="ml-3 text-[11px] text-slate-400">
          {step.sub_steps.map((sub, i) => (
            <li key={i}>
              <Parts parts={sub} />
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

export function RoutePanel(): React.JSX.Element {
  const route = useAppStore((s) => s.route);
  const grabbed = useAppStore((s) => s.ui.grabbed);

  if (!route) {
    return <div className="text-xs text-slate-400">No route loaded.</div>;
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between text-[10px] uppercase tracking-wider text-slate-400">
        <span>
          {route.section} · {route.edge + 1}/{route.edge_count}
        </span>
        {grabbed && (
          <span className="flex gap-1 normal-case">
            <button className="rounded bg-slate-700/60 px-1.5" onClick={() => void routePrev()}>
              Back
            </button>
            <button className="rounded bg-slate-700/60 px-1.5" onClick={() => void routeNext()}>
              Next
            </button>
          </span>
        )}
      </div>
      <ul className="flex flex-col gap-0.5 text-xs">
        {route.steps.map((step, i) => (
          <StepView key={i} step={step} />
        ))}
      </ul>
      {route.next_steps.length > 0 && (
        <ul className="border-t border-slate-700/50 pt-1 text-[11px] text-slate-500">
          <StepView step={route.next_steps[0]} />
        </ul>
      )}
    </div>
  );
}
