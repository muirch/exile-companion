import type { CSSProperties, ReactNode } from 'react';

// The band's end: lines from both corners meet at a point on the middle.
function End({ side }: { side: 'start' | 'end' }): React.JSX.Element {
  return (
    <span className={`end end-${side}`} aria-hidden="true">
      <svg viewBox="0 0 14 100" preserveAspectRatio="none" className="size-full">
        <path
          d="M0 0L11 50L0 100"
          fill="none"
          stroke="currentColor"
          strokeWidth="1"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
      <span className="gem" />
    </span>
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
      <End side="start" />
      <h1 className="truncate text-[19px] leading-tight font-medium">{title}</h1>
      {subtitle && <p className="text-[14px] leading-tight opacity-85">{subtitle}</p>}
      <End side="end" />
    </header>
  );
}

export function TooltipSep({ tint }: { tint: string }): React.JSX.Element {
  return <div className="tooltip-sep" style={{ '--tint': tint } as CSSProperties} />;
}
