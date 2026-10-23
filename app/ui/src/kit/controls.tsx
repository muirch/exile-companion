import { useEffect, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react';

import { TooltipHeader } from '../components/Tooltip';

/** `literal` sets the label in Alegreya Sans, for key names, speeds and other literal strings. */
export function Button({
  literal = false,
  className = '',
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { literal?: boolean }): React.JSX.Element {
  return (
    <button
      type="button"
      className={['btn', literal && 'font-body', className].filter(Boolean).join(' ')}
      {...props}
    />
  );
}

// Drops progress, the build or the game log: the first click arms it, the second does it.
// A second click right after the first is a double click or a bouncing mouse button, not a
// confirmation.
const CONFIRM_AFTER_MS = 500;

export function ConfirmButton({
  label,
  confirm,
  onConfirm,
}: {
  label: string;
  confirm: string;
  onConfirm: () => void;
}): React.JSX.Element {
  const [armed, setArmed] = useState(false);
  const armedAt = useRef(0);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(t);
  }, [armed]);
  return (
    <Button
      aria-pressed={armed}
      onClick={() => {
        if (!armed) {
          armedAt.current = Date.now();
          setArmed(true);
        } else if (Date.now() - armedAt.current >= CONFIRM_AFTER_MS) {
          setArmed(false);
          onConfirm();
        }
      }}
    >
      {armed ? confirm : label}
    </Button>
  );
}

export function Toggle({
  on,
  onChange,
  label,
}: {
  on: boolean;
  onChange: (v: boolean) => void;
  label?: string;
}): React.JSX.Element {
  return (
    <Button role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)}>
      {on ? 'On' : 'Off'}
    </Button>
  );
}

/** A row of buttons where the pressed one is the current value. */
export function Choice<T extends string | number | boolean>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string; literal?: boolean }[];
  onChange: (v: T) => void;
}): React.JSX.Element {
  return (
    <>
      {options.map((o) => (
        <Button
          key={String(o.value)}
          literal={o.literal}
          aria-pressed={value === o.value}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </Button>
      ))}
    </>
  );
}

/** Range input with its value as a percentage next to it. */
export function Slider({
  value,
  min,
  max,
  step,
  label,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  label: string;
  onChange: (v: number) => void;
}): React.JSX.Element {
  return (
    <>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        aria-label={label}
        onChange={(e) => onChange(Number(e.target.value))}
        className="flex-1"
      />
      <span className="w-11 text-right text-[13px] text-value">{Math.round(value * 100)}%</span>
    </>
  );
}

// Each group is its own item tooltip: a header band over a see-through black body.
export function Section({ title, children }: { title: string; children: ReactNode }): React.JSX.Element {
  return (
    <section className="tooltip bg-black/80">
      <TooltipHeader tint="var(--color-currency)" title={title} />
      <div className="flex flex-col gap-2 px-5 pt-3 pb-4 text-left">{children}</div>
    </section>
  );
}

export function Row({
  label,
  className = '',
  children,
}: {
  label: string;
  className?: string;
  children: ReactNode;
}): React.JSX.Element {
  return (
    <div className={`flex min-h-7 items-center gap-3 ${className}`}>
      <span className="w-32 shrink-0 font-caps text-[13px] text-label">{label}</span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">{children}</div>
    </div>
  );
}

/** Caption under a row, lined up with the row's controls unless `flush`. */
export function Note({ flush = false, children }: { flush?: boolean; children: ReactNode }): React.JSX.Element {
  return (
    <p className={`${flush ? '' : 'ml-35 '}font-body text-[12px] leading-snug text-label italic`}>
      {children}
    </p>
  );
}
