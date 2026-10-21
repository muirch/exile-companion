import type { CSSProperties, ReactNode } from 'react';

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
      <span className="gem gem-start" aria-hidden="true" />
      <h1 className="truncate text-[19px] leading-tight font-medium">{title}</h1>
      {subtitle && <p className="text-[14px] leading-tight opacity-85">{subtitle}</p>}
      <span className="gem gem-end" aria-hidden="true" />
    </header>
  );
}

export function TooltipSep({ tint }: { tint: string }): React.JSX.Element {
  return <div className="tooltip-sep" style={{ '--tint': tint } as CSSProperties} />;
}
