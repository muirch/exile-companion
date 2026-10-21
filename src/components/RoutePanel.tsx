import { routeNext, routePrev } from '../ipc/commands';
import type { Part, Step } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';

// 0 is north, clockwise in 45 degree steps (exile-leveling's convention).
function DirArrow({ degrees }: { degrees: number }): React.JSX.Element {
  return (
    <svg
      viewBox="0 0 12 12"
      className="mx-0.5 inline-block size-3.5 -translate-y-px align-middle text-gold"
      style={{ transform: `rotate(${degrees}deg)` }}
      role="img"
      aria-label={`direction ${degrees} degrees`}
    >
      <path d="M6 1 10 7H7.2v4H4.8V7H2Z" fill="currentColor" />
    </svg>
  );
}

// The route marks moving on with "➞ "; draw it instead of trusting a fallback font for the glyph.
function GoMark(): React.JSX.Element {
  return (
    <svg
      viewBox="0 0 12 12"
      className="mr-1 inline-block size-3 -translate-y-px align-middle text-bronze-bright"
      aria-hidden="true"
    >
      <path d="M1 6h7M5.5 2.5 9 6l-3.5 3.5" fill="none" stroke="currentColor" strokeWidth="1.6" />
    </svg>
  );
}

function RouteText({ text }: { text: string }): React.JSX.Element {
  // "3x{quest_text|Glyph}" in the route reads "3xGlyph" without this space.
  const spaced = /\dx$/.test(text) ? `${text} ` : text;
  const pieces = spaced.split('➞');
  return (
    <>
      {pieces.map((piece, i) => (
        <span key={i}>
          {i > 0 && <GoMark />}
          {i > 0 ? piece.trimStart() : piece}
        </span>
      ))}
    </>
  );
}

function PartView({ part }: { part: Part }): React.JSX.Element {
  switch (part.kind) {
    case 'text':
      return <RouteText text={part.text} />;
    case 'kill':
      return <span className="font-medium text-blood">{part.name}</span>;
    case 'arena':
      return <span className="text-trial">{part.name}</span>;
    case 'area':
    case 'enter':
      return <span className="text-gold">{part.name}</span>;
    case 'logout':
      return (
        <span>
          <span className="text-slate">Logout</span> to <span className="text-gold">{part.town}</span>
        </span>
      );
    case 'waypoint':
    case 'waypoint_get':
      return <span className="text-slate">waypoint</span>;
    case 'waypoint_use':
      return (
        <span>
          <span className="text-slate">Waypoint</span> to <span className="text-gold">{part.name}</span>
        </span>
      );
    case 'portal_set':
      return <span className="text-slate">portal</span>;
    case 'portal_use':
      return (
        <span>
          <span className="text-slate">portal</span> to <span className="text-gold">{part.name}</span>
        </span>
      );
    case 'quest':
      return <span className="text-quest">{part.name}</span>;
    case 'quest_text':
      return <span className="text-quest">{part.text}</span>;
    case 'generic':
      return <span className="text-ash">{part.text}</span>;
    case 'reward_quest':
    case 'reward_vendor':
      return <span className="text-gem">{part.item}</span>;
    case 'trial':
      return <span className="text-trial">Trial of Ascendancy</span>;
    case 'ascend':
      return <span className="text-trial">Ascend in the {part.difficulty} Labyrinth</span>;
    case 'crafting':
      return <span className="text-craft">crafting recipe</span>;
    case 'dir':
      return <DirArrow degrees={part.degrees} />;
    case 'copy':
      return <span className="text-ash">{part.text}</span>;
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
    <li>
      <Parts parts={step.parts} />
      {step.sub_steps.length > 0 && (
        <ul className="mt-0.5 ml-3 text-[12px] leading-snug text-ash-dim">
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
    return <p className="text-center text-[13px] text-ash-dim">The leveling route didn't load.</p>;
  }

  return (
    <div className="flex flex-col gap-2">
      <ul className="flex flex-col gap-1 text-[14px] leading-snug">
        {route.steps.map((step, i) => (
          <StepView key={i} step={step} />
        ))}
      </ul>
      {route.next_steps.length > 0 && (
        <>
          <div className="separator" />
          <ul className="text-[12px] leading-snug opacity-70">
            <StepView step={route.next_steps[0]} />
          </ul>
        </>
      )}
      {grabbed && (
        <div className="flex items-center justify-end gap-1.5 pt-0.5">
          <span className="mr-auto text-[11px] text-ash-faint">
            Ctrl+Alt+PageUp and PageDown also step the route
          </span>
          <button className="btn" onClick={() => void routePrev()}>
            Back
          </button>
          <button className="btn" onClick={() => void routeNext()}>
            Next
          </button>
        </div>
      )}
    </div>
  );
}
