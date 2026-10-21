import type { CSSProperties, ReactNode } from 'react';

// Our own scrollwork; the game's header art isn't ours to ship.
function Corner({ end }: { end?: boolean }): React.JSX.Element {
  return (
    <svg
      viewBox="0 0 24 30"
      className={`corner ${end ? 'corner-end' : 'corner-start'}`}
      aria-hidden="true"
    >
      <g fill="none" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round">
        <path d="M3 3C9 6 11 10 11 15C11 20 9 24 3 27" />
        <path d="M11 15C11 10 15 8 18 10C20 11.5 19.5 14 17.5 14C16 14 15.5 12.8 16.3 12" />
        <path d="M11 15C11 20 15 22 18 20C20 18.5 19.5 16 17.5 16C16 16 15.5 17.2 16.3 18" />
      </g>
      <path d="M4 15l2-2 2 2-2 2Z" fill="currentColor" />
    </svg>
  );
}

export function TooltipHeader({
  tint,
  title,
  subtitle,
}: {
  tint: string;
  title: ReactNode;
  subtitle?: ReactNode;
}): React.JSX.Element {
  return (
    <header className="tooltip-header" style={{ '--tint': tint } as CSSProperties}>
      <Corner />
      <h1 className="truncate text-[19px] leading-tight font-medium">{title}</h1>
      {subtitle && <p className="text-[14px] leading-tight opacity-85">{subtitle}</p>}
      <Corner end />
    </header>
  );
}

export function TooltipSep({ tint }: { tint: string }): React.JSX.Element {
  return <div className="tooltip-sep" style={{ '--tint': tint } as CSSProperties} />;
}
