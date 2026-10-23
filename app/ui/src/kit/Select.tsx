import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

const LIST_MAX_HEIGHT = 240;

// Pointer handling sits on the list, the options only carry their index.
function optionAt(target: EventTarget): number | null {
  const el = (target as Element).closest<HTMLElement>('[role="option"]');
  return el ? Number(el.dataset.index) : null;
}

// WebView2 draws the native select popup with a white system background, so the list is our own.
// It follows the ARIA select-only combobox: focus stays on the button, the list tracks the
// highlighted option through aria-activedescendant.
export function Select<T extends string>({
  value,
  options,
  onChange,
  label,
  className = '',
}: {
  value: T;
  options: SelectOption<T>[];
  onChange: (v: T) => void;
  label: string;
  className?: string;
}): React.JSX.Element {
  const id = useId();
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [place, setPlace] = useState<{ left: number; top: number; width: number }>();
  const selected = Math.max(0, options.findIndex((o) => o.value === value));

  const show = (at = selected): void => {
    setActive(at);
    setOpen(true);
  };
  const close = (): void => setOpen(false);
  const pick = (i: number): void => {
    close();
    if (options[i].value !== value) onChange(options[i].value);
  };

  // Fixed, so the scrolling settings column can't clip it. It opens upward when there's no room below.
  useLayoutEffect(() => {
    if (!open || !button.current) return;
    const r = button.current.getBoundingClientRect();
    const height = Math.min(list.current?.scrollHeight ?? 0, LIST_MAX_HEIGHT);
    const up = window.innerHeight - r.bottom < height + 8 && r.top > window.innerHeight - r.bottom;
    setPlace({ left: r.left, top: up ? r.top - height - 2 : r.bottom + 2, width: r.width });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    list.current?.children[active]?.scrollIntoView({ block: 'nearest' });
  }, [open, active]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent): void => {
      const t = e.target as Node;
      if (!button.current?.contains(t) && !list.current?.contains(t)) setOpen(false);
    };
    const onScroll = (e: Event): void => {
      if (!list.current?.contains(e.target as Node)) setOpen(false);
    };
    const onResize = (): void => setOpen(false);
    document.addEventListener('pointerdown', onDown);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onResize);
    return () => {
      document.removeEventListener('pointerdown', onDown);
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onResize);
    };
  }, [open]);

  const onKey = (e: KeyboardEvent): void => {
    const last = options.length - 1;
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    const move: Record<string, number> = {
      ArrowDown: Math.min(active + 1, last),
      ArrowUp: Math.max(active - 1, 0),
      Home: 0,
      End: last,
      PageDown: Math.min(active + 5, last),
      PageUp: Math.max(active - 5, 0),
    };
    if (e.key in move) {
      e.preventDefault();
      setActive(move[e.key]);
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      pick(active);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'Tab') {
      close();
    } else if (e.key.length === 1) {
      const ch = e.key.toLowerCase();
      const next = options.findIndex((o, i) => i > active && o.label.toLowerCase().startsWith(ch));
      const first = options.findIndex((o) => o.label.toLowerCase().startsWith(ch));
      if (next >= 0 || first >= 0) setActive(next >= 0 ? next : first);
    }
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        role="combobox"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={`${id}-list`}
        aria-activedescendant={open ? `${id}-${active}` : undefined}
        className={`btn select font-body ${className}`}
        onClick={() => (open ? close() : show())}
        onKeyDown={onKey}
      >
        <span className="min-w-0 flex-1 truncate text-left">{options[selected]?.label}</span>
        <svg viewBox="0 0 8 5" className="select-chevron" aria-hidden="true">
          <path d="M.5.5 4 4 7.5.5" fill="none" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      </button>
      {open && (
        <div
          ref={list}
          id={`${id}-list`}
          role="listbox"
          aria-label={label}
          tabIndex={-1}
          onKeyDown={onKey}
          className="select-list"
          style={{
            left: place?.left ?? 0,
            top: place?.top ?? 0,
            minWidth: place?.width,
            maxHeight: LIST_MAX_HEIGHT,
            visibility: place ? 'visible' : 'hidden',
          }}
          onPointerMove={(e) => {
            const i = optionAt(e.target);
            if (i !== null && i !== active) setActive(i);
          }}
          onPointerDown={(e) => e.preventDefault()}
          onClick={(e) => {
            const i = optionAt(e.target);
            if (i !== null) pick(i);
          }}
        >
          {options.map((o, i) => (
            <div
              key={o.value}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === selected}
              data-index={i}
              data-active={i === active || undefined}
            >
              {o.label}
            </div>
          ))}
        </div>
      )}
    </>
  );
}
