import type { CSSProperties, ReactNode } from 'react';

// Our own scrollwork; the game's header art isn't ours to ship.
function Corner({ end }: { end?: boolean }): React.JSX.Element {
  return (
    <svg
      viewBox="0 0 40 34"
      preserveAspectRatio="xMinYMid meet"
      className={`corner ${end ? 'corner-end' : 'corner-start'}`}
      aria-hidden="true"
    >
      <g fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
        <path d="M4 3C11 7 14 11 14 17C14 23 11 27 4 31" />
        <path d="M14 17C14 11 19 8 24 9.5C28 10.8 28.5 15 25.5 16C23.5 16.6 22 15.3 22.6 13.8" />
        <path d="M14 17C14 23 19 26 24 24.5C28 23.2 28.5 19 25.5 18C23.5 17.4 22 18.7 22.6 20.2" />
        <path d="M26 17H38" opacity="0.7" />
      </g>
      <circle cx="22.6" cy="13.8" r="1.4" fill="currentColor" />
      <circle cx="22.6" cy="20.2" r="1.4" fill="currentColor" />
      <path d="M6 17l2.5-2.5L11 17l-2.5 2.5Z" fill="currentColor" />
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
