import type { Part, Step } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';

// 0 is north, clockwise in 45 degree steps (exile-leveling's convention).
function DirArrow({ degrees }: { degrees: number }): React.JSX.Element {
  return (
    <svg
      viewBox="0 0 12 12"
      className="mx-0.5 inline-block size-3.5 -translate-y-px align-middle text-value"
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
      className="mr-1 inline-block size-3 -translate-y-px align-middle text-plain"
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
      return <span className="text-corrupted">{part.name}</span>;
    case 'arena':
      return <span className="text-unique">{part.name}</span>;
    case 'area':
    case 'enter':
      return <span className="text-value">{part.name}</span>;
    case 'logout':
      return (
        <span>
          <span className="text-magic">Logout</span> to <span className="text-value">{part.town}</span>
        </span>
      );
    case 'waypoint':
    case 'waypoint_get':
      return <span className="text-magic">Waypoint</span>;
    case 'waypoint_use':
      return (
        <span>
          <span className="text-magic">Waypoint</span> to <span className="text-value">{part.name}</span>
        </span>
      );
    case 'portal_set':
      return <span className="text-magic">Portal</span>;
    case 'portal_use':
      return (
        <span>
          <span className="text-magic">Portal</span> to <span className="text-value">{part.name}</span>
        </span>
      );
    case 'quest':
      return <span className="text-quest">{part.name}</span>;
    case 'quest_text':
      return <span className="text-quest">{part.text}</span>;
    case 'generic':
      return <span className="text-value">{part.text}</span>;
    case 'reward_quest':
    case 'reward_vendor':
      return <span className="text-gem">{part.item}</span>;
    case 'trial':
      return <span className="text-unique">Trial of Ascendancy</span>;
    case 'ascend':
      return <span className="text-unique">Ascend in the {part.difficulty} Labyrinth</span>;
    case 'crafting':
      return <span className="text-crafted">Crafting recipe</span>;
    case 'dir':
      return <DirArrow degrees={part.degrees} />;
    case 'copy':
      return <span className="text-value">{part.text}</span>;
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

// Hints are the game's flavour-text voice: grey italic, not small caps.
function Hints({ hints }: { hints: Part[][] }): React.JSX.Element | null {
  if (hints.length === 0) return null;
  return (
    <ul className="mt-0.5 font-body text-[13px] leading-snug text-plain italic">
      {hints.map((h, i) => (
        <li key={i}>
          <Parts parts={h} />
        </li>
      ))}
    </ul>
  );
}

function StepView({ step, hints }: { step: Step; hints: boolean }): React.JSX.Element {
  return (
    <li>
      {step.parts.length > 0 && <Parts parts={step.parts} />}
      {hints && <Hints hints={step.sub_steps} />}
    </li>
  );
}

export function HereSteps({ hints = true }: { hints?: boolean }): React.JSX.Element {
  const route = useAppStore((s) => s.route);
  if (!route) {
    return (
      <p className="text-[14px] text-plain">
        The leveling route didn't load. Restart poebo; if it stays, reinstall it.
      </p>
    );
  }
  return (
    <>
      <ul className="flex flex-col gap-1 text-[15px] leading-snug text-value">
        {route.here.map((step, i) => (
          <StepView key={i} step={step} hints={hints} />
        ))}
      </ul>
    </>
  );
}

export function hasActions(route: { here: Step[] } | null): boolean {
  return !!route && route.here.some((s) => s.parts.length > 0);
}

export function ThenStep(): React.JSX.Element | null {
  const then = useAppStore((s) => s.route?.then);
  if (!then) return null;
  // "Then:" already says what comes next, so a leading arrow is dropped.
  const [first, ...rest] = then.parts;
  const parts =
    first?.kind === 'text' && first.text.trimStart().startsWith('➞')
      ? [{ ...first, text: first.text.trimStart().slice(1).trimStart() }, ...rest]
      : then.parts;
  return (
    <div className="text-[14px] leading-snug">
      <span className="text-plain">Then: </span>
      <Parts parts={parts} />
    </div>
  );
}

// When the area is only a passage, the way out is the instruction.
export function PassThrough(): React.JSX.Element | null {
  const then = useAppStore((s) => s.route?.then);
  return (
    <>
      <p className="text-[13px] text-plain">Nothing to do here</p>
      {then && (
        <p className="text-[15px] leading-snug text-value">
          <Parts parts={then.parts} />
        </p>
      )}
    </>
  );
}
