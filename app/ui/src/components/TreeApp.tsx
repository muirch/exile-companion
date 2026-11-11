import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';

import { closeTree, getTreeLayout, getTreeStage, setTreeMode } from '../ipc/commands';
import { isTauri } from '../ipc/env';
import { onTreeOpened } from '../ipc/events';
import type { Mark, NodeKind, TreeLayout, TreeMode, TreeNode, TreeStage } from '../ipc/types';
import { Button, Choice, Select } from '../kit';
import { Brand } from './Tooltip';
import {
  bounds,
  circlesPath,
  edgePath,
  fit,
  nodeAt,
  panBy,
  placeLabels,
  RADIUS,
  toScreen,
  zoomAt,
  type View,
} from './treeView';

const COLOR: Record<Mark, string> = {
  taken: 'var(--color-normal)',
  added: 'var(--color-quest)',
  extra: 'var(--color-corrupted)',
};
const OFF_NODE = '#1e1e1e';
const OFF_RING = '#4a4a4a';
const OFF_EDGE = '#2a2a2a';
const KIND: Record<NodeKind, string> = {
  normal: 'Passive',
  notable: 'Notable',
  keystone: 'Keystone',
  jewel: 'Jewel socket',
  mastery: 'Mastery',
  ascendancyStart: 'Ascendancy',
};
const CARD_WIDTH = 250;
const INSET = 150;
// The corner map is drawn small, so its nodes are drawn bigger than on the tree.
const INSET_GROW = 1.8;

const modes = (canCompare: boolean): { value: TreeMode; label: string; disabled?: boolean }[] => [
  { value: 'tree', label: 'Tree' },
  { value: 'new', label: 'New' },
  { value: 'character', label: 'Vs. character', disabled: !canCompare },
];

/** What the mode calls a mark, for the legend and the card. */
export function markName(mode: TreeMode, mark: Mark): string {
  if (mark === 'taken') return 'Taken';
  if (mode === 'character') return mark === 'added' ? 'To take' : 'Not in the guide';
  return mark === 'added' ? 'New at this stage' : 'Refunded at this stage';
}

const count = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;

/** The line over the tree: what the stage has, or what changes. */
export function summary(s: TreeStage): React.ReactNode {
  const names = s.addedNames.length ? `, notables ${s.addedNames.join(', ')}` : '';
  if (s.mode === 'tree') return `${count(s.points, 'point', 'points')}, ${count(s.notables, 'notable', 'notables')}.`;
  if (s.mode === 'new') {
    if (s.stage === 0) return `The first stage, ${count(s.points, 'point', 'points')}${names}.`;
    return (
      <>
        <span className="text-quest">{s.added} new</span> since {s.stages[s.stage - 1]}
        {names}.{s.extra > 0 && <> <span className="text-corrupted">{s.extra} refunded</span>.</>}
      </>
    );
  }
  if (s.added === 0 && s.extra === 0) return 'Your tree has everything this stage takes.';
  return (
    <>
      <span className="text-quest">{s.added} to take</span> for {s.stages[s.stage]}
      {names}.
      {s.extra > 0 && (
        <>
          {' '}
          <span className="text-corrupted">{count(s.extra, 'point', 'points')}</span> the guide
          doesn&apos;t take.
        </>
      )}
    </>
  );
}

function Ascendancy({ stage }: { stage: TreeStage }): React.JSX.Element | null {
  if (stage.ascendancy.length === 0) return null;
  return (
    <span className="font-body text-[13px] text-plain italic">
      {' '}
      Ascendancy:{' '}
      {stage.ascendancy.map((a, i) => (
        <span key={a.name} className={a.mark === 'taken' ? undefined : a.mark === 'added' ? 'text-quest' : 'text-corrupted'}>
          {i > 0 && ', '}
          {a.name}
        </span>
      ))}
    </span>
  );
}

function Swatch({ mark }: { mark: Mark }): React.JSX.Element {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" className="mr-1 inline-block align-[-2px]">
      {mark === 'taken' ? (
        <circle cx="7" cy="7" r="4" fill={COLOR.taken} />
      ) : (
        <circle cx="7" cy="7" r="3.5" fill="#0a0a0a" stroke={COLOR[mark]} strokeWidth="2" />
      )}
      {mark === 'extra' && <path d="M2 2L12 12" stroke={COLOR.extra} strokeWidth="1.5" />}
    </svg>
  );
}

function Legend({ stage }: { stage: TreeStage }): React.JSX.Element {
  const marks: Mark[] = stage.mode === 'tree' ? [] : stage.extra > 0 ? ['taken', 'added', 'extra'] : ['taken', 'added'];
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-x-3.5 px-3 pt-1.5 pb-2 font-body text-[11px] text-label">
      {marks.map((m) => (
        <span key={m}>
          <Swatch mark={m} />
          {markName(stage.mode, m)}
        </span>
      ))}
      <span className="ml-auto">Drag to move, wheel to zoom</span>
    </div>
  );
}

// The browser mock opens a card with ?hover, for screenshots.
function mockHover(layout: TreeLayout, stage: TreeStage): Hover | null {
  const kind = new URLSearchParams(window.location.search).get('hover');
  if (isTauri() || kind === null) return null;
  const want = (n: TreeNode | undefined): boolean => (kind === 'mastery' ? n?.kind === 'mastery' : n?.kind === 'notable' && !n.ascendancy);
  const marked = stage.nodes.find((m) => (kind === 'mastery' || m.mark === (stage.mode === 'tree' ? 'taken' : 'added')) && want(layout.nodes.find((n) => n.id === m.id)));
  const node = layout.nodes.find((n) => n.id === marked?.id);
  return node ? { node } : null;
}

interface Hover {
  node: TreeNode;
  /** Where an ascendancy node sits on screen. Main tree nodes follow the view. */
  at?: { x: number; y: number };
}

/** A node the stage marks, filled when taken, ringed when added, slashed when extra. */
function MarkedNode({
  n,
  mark,
  px,
  grow = 1,
}: {
  n: TreeNode;
  mark: Mark;
  px: (v: number) => number;
  grow?: number;
}): React.JSX.Element {
  const r = RADIUS[n.kind] * grow;
  const fill = mark === 'taken' ? COLOR.taken : '#0a0a0a';
  const stroke = mark === 'taken' ? 'var(--color-value)' : COLOR[mark];
  const sw = px(mark === 'taken' ? 1 : 2);
  const cross = r + px(2.5);
  return (
    <g data-node={n.id} data-mark={mark}>
      {n.kind === 'jewel' ? (
        <rect x={n.x - r} y={n.y - r} width={r * 2} height={r * 2} transform={`rotate(45 ${n.x} ${n.y})`} fill={fill} stroke={stroke} strokeWidth={sw} />
      ) : (
        <circle cx={n.x} cy={n.y} r={r} fill={fill} stroke={stroke} strokeWidth={sw} />
      )}
      {mark === 'extra' && (
        <path d={`M${n.x - cross} ${n.y - cross}L${n.x + cross} ${n.y + cross}`} stroke={COLOR.extra} strokeWidth={px(1.5)} />
      )}
    </g>
  );
}

type Marks = Map<number, TreeStage['nodes'][number]>;

function edgePaths(layout: TreeLayout, byId: Map<number, TreeNode>, edgeMarks: Map<number, Mark>, keep: (n: TreeNode) => boolean): Record<Mark | 'off', string> {
  const out: Record<Mark | 'off', string> = { off: '', taken: '', added: '', extra: '' };
  layout.edges.forEach((e, i) => {
    const a = byId.get(e.a);
    const b = byId.get(e.b);
    if (a && b && keep(a)) out[edgeMarks.get(i) ?? 'off'] += edgePath(a, b, e);
  });
  return out;
}

function Edges({ paths, px }: { paths: Record<Mark | 'off', string>; px: (v: number) => number }): React.JSX.Element {
  return (
    <>
      <path d={paths.off} fill="none" stroke={OFF_EDGE} strokeWidth={px(1)} />
      {(['taken', 'extra', 'added'] as Mark[]).map((m) =>
        paths[m] ? (
          <path
            key={m}
            data-edges={m}
            d={paths[m]}
            fill="none"
            stroke={COLOR[m]}
            strokeWidth={px(2)}
            strokeDasharray={m === 'added' ? `${px(4)} ${px(3)}` : undefined}
          />
        ) : null,
      )}
    </>
  );
}

/**
 * An ascendancy drawn apart in the corner, the way the game opens it in its own panel. It doesn't
 * move with the tree.
 */
function AscendancyMap({
  name,
  layout,
  byId,
  marks,
  edgeMarks,
  left,
  top,
  hover,
  onHover,
}: {
  name: string;
  layout: TreeLayout;
  byId: Map<number, TreeNode>;
  marks: Marks;
  edgeMarks: Map<number, Mark>;
  left: number;
  top: number;
  hover: Hover | null;
  onHover: (h: Hover | null) => void;
}): React.JSX.Element | null {
  const nodes = useMemo(() => layout.nodes.filter((n) => n.ascendancy === name), [layout, name]);
  const paths = useMemo(() => edgePaths(layout, byId, edgeMarks, (n) => n.ascendancy === name), [layout, byId, edgeMarks, name]);
  const b = bounds(nodes);
  if (!b) return null;
  const v = fit(b, INSET, INSET, 160);
  const px = (x: number): number => x / v.scale;
  const onMove = (e: React.PointerEvent<HTMLDivElement>): void => {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    const n = nodeAt(nodes, v, e.clientX - r.left, e.clientY - r.top);
    if (!n) return onHover(null);
    if (n.id === hover?.node.id) return;
    const s = toScreen(v, n.x, n.y);
    onHover({ node: n, at: { x: left + s.x, y: top + s.y } });
  };
  return (
    <div
      data-testid={`ascendancy-${name}`}
      className="absolute cursor-default border border-edge bg-[#050505]"
      style={{ left, top, width: INSET, height: INSET }}
      onPointerDown={(e) => e.stopPropagation()}
      onPointerMove={onMove}
      onWheel={(e) => e.stopPropagation()}
    >
      <svg width={INSET} height={INSET} viewBox={`${v.x} ${v.y} ${INSET / v.scale} ${INSET / v.scale}`} className="absolute inset-0">
        <Edges paths={paths} px={px} />
        {nodes.map((n) => {
          const m = marks.get(n.id);
          if (m) return <MarkedNode key={n.id} n={n} mark={m.mark} px={px} grow={INSET_GROW} />;
          return (
            <circle key={n.id} cx={n.x} cy={n.y} r={RADIUS[n.kind] * INSET_GROW} fill={OFF_NODE} stroke={OFF_RING} strokeWidth={px(1)} />
          );
        })}
        {hover?.node.ascendancy === name && (
          <circle
            cx={hover.node.x}
            cy={hover.node.y}
            r={RADIUS[hover.node.kind] * INSET_GROW + px(4)}
            fill="none"
            stroke="var(--color-value)"
            strokeWidth={px(1)}
          />
        )}
      </svg>
      <span className="pointer-events-none absolute top-1 left-1.5 font-caps text-[11px] text-label">{name}</span>
    </div>
  );
}

function NodeCard({
  hover,
  stage,
  mark,
  since,
  effect,
  at,
  box,
}: {
  hover: Hover;
  stage: TreeStage;
  mark: Mark | undefined;
  since: number | null | undefined;
  effect: number | null | undefined;
  at: { x: number; y: number };
  box: { w: number; h: number };
}): React.JSX.Element {
  const { node } = hover;
  const stats = node.mastery?.find((m) => m.effect === effect)?.stats ?? node.stats;
  const r = 10;
  const left = at.x + r + CARD_WIDTH > box.w ? Math.max(0, at.x - r - CARD_WIDTH) : at.x + r;
  const below = at.y < box.h / 2;
  const kind = [KIND[node.kind], mark && stage.mode !== 'tree' && mark !== 'taken' ? markName(stage.mode, mark) : null]
    .filter(Boolean)
    .join(' · ');
  let where: string | null = null;
  if (since != null && mark !== 'extra') where = `The guide takes it at ${stage.stages[since]}.`;
  else if (mark === 'extra' && stage.mode === 'character') where = "The guide doesn't take it.";
  return (
    <div
      id="node-card"
      role="tooltip"
      className="pointer-events-none absolute z-10 flex flex-col gap-0.5 border border-edge bg-[#0a0a0a] px-3 py-2"
      style={{ left, width: CARD_WIDTH, ...(below ? { top: at.y + r } : { bottom: box.h - at.y + r }) }}
    >
      <p className="font-caps text-[15px] leading-snug text-value">{node.name}</p>
      <p className="font-caps text-[13px] text-label">{kind}</p>
      {stats.length > 0 && (
        <p className="mt-1 font-body text-[13px] leading-snug whitespace-pre-line text-plain">{stats.join('\n')}</p>
      )}
      {where && <p className="mt-1 font-body text-[13px] leading-snug text-plain italic">{where}</p>}
    </div>
  );
}

function TreeCanvas({ layout, stage }: { layout: TreeLayout; stage: TreeStage }): React.JSX.Element {
  const box = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  // Until the player moves the tree it stays fitted, also when the window is resized.
  const [moved, setMoved] = useState<View | null>(null);
  const [hover, setHover] = useState<Hover | null>(() => mockHover(layout, stage));
  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);

  const byId = useMemo(() => new Map(layout.nodes.map((n) => [n.id, n])), [layout]);
  const marks = useMemo(() => new Map(stage.nodes.map((n) => [n.id, n])), [stage]);
  const edgeMarks = useMemo(() => new Map(stage.edges.map((e) => [e.edge, e.mark])), [stage]);
  const main = useMemo(() => layout.nodes.filter((n) => !n.ascendancy), [layout]);
  // An unpicked mastery isn't drawn, so it can't be hovered either.
  const hoverable = useMemo(() => main.filter((n) => n.kind !== 'mastery' || marks.has(n.id)), [main, marks]);
  const focus = useMemo(() => stage.focus ?? bounds(main), [stage, main]);
  const paths = useMemo(() => edgePaths(layout, byId, edgeMarks, (n) => !n.ascendancy), [layout, byId, edgeMarks]);
  const offNodes = useMemo(
    () => circlesPath(main.filter((n) => !marks.has(n.id) && n.kind !== 'mastery' && n.kind !== 'jewel')),
    [main, marks],
  );
  const offJewels = useMemo(() => main.filter((n) => !marks.has(n.id) && n.kind === 'jewel'), [main, marks]);

  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = (): void => setSize({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const fitted = useMemo(() => (focus && size.w > 0 ? fit(focus, size.w, size.h) : null), [focus, size]);
  const view = moved ?? fitted;
  const setView = (v: View | null): void => {
    setMoved(v);
    setHover(null);
  };

  useEffect(() => {
    if (!view) return;
    const step = 60;
    const centre = (f: number): View => zoomAt(view, f, size.w / 2, size.h / 2);
    const onKey = (e: KeyboardEvent): void => {
      // The stage select and the buttons keep their own keys.
      if ((e.target as HTMLElement).closest?.('button, input, [role="listbox"]')) return;
      const moves: Record<string, () => View | null> = {
        '+': () => centre(1.25),
        '=': () => centre(1.25),
        '-': () => centre(0.8),
        '0': () => null,
        ArrowLeft: () => panBy(view, step, 0),
        ArrowRight: () => panBy(view, -step, 0),
        ArrowUp: () => panBy(view, 0, step),
        ArrowDown: () => panBy(view, 0, -step),
      };
      const move = moves[e.key];
      if (!move) return;
      e.preventDefault();
      setMoved(move());
      setHover(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [view, size]);

  const point = (e: { clientX: number; clientY: number }): { x: number; y: number } => {
    const r = box.current?.getBoundingClientRect();
    return { x: e.clientX - (r?.left ?? 0), y: e.clientY - (r?.top ?? 0) };
  };

  const onWheel = (e: React.WheelEvent): void => {
    if (!view) return;
    const p = point(e);
    setView(zoomAt(view, Math.pow(1.0015, -e.deltaY), p.x, p.y));
  };

  const onPointerDown = (e: React.PointerEvent): void => {
    drag.current = { ...point(e), moved: false };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent): void => {
    if (!view) return;
    const p = point(e);
    const d = drag.current;
    if (d && (d.moved || Math.hypot(p.x - d.x, p.y - d.y) > 3)) {
      setView(panBy(view, p.x - d.x, p.y - d.y));
      drag.current = { ...p, moved: true };
      return;
    }
    const n = nodeAt(hoverable, view, p.x, p.y);
    if (!n) setHover(null);
    else if (n.id !== hover?.node.id || hover.at) setHover({ node: n });
  };

  const labels = useMemo(() => {
    if (!view) return [];
    const wanted = stage.nodes
      .map((m) => ({ m, n: byId.get(m.id) }))
      .filter(({ m, n }) =>
        n && n.name && !n.ascendancy && (n.kind === 'keystone' || (n.kind === 'notable' && (stage.mode === 'tree' || m.mark === 'added'))),
      )
      .map(({ m, n }) => {
        const s = toScreen(view, n!.x, n!.y);
        return { id: m.id, text: n!.name, x: s.x, y: s.y, r: RADIUS[n!.kind] * view.scale, mark: m.mark, kind: n!.kind };
      });
    // What's still to take and keystones win the room first.
    const rank = (l: (typeof wanted)[number]): number => (l.mark === 'added' ? 0 : 2) + (l.kind === 'keystone' ? 0 : 1);
    const placed = placeLabels(
      [0, 1, 2, 3].flatMap((r) => wanted.filter((l) => rank(l) === r)),
      size.w,
      size.h,
    );
    return placed;
  }, [view, stage, byId, size]);

  const px = (v: number): number => (view ? v / view.scale : v);
  const viewBox = view ? `${view.x} ${view.y} ${size.w / view.scale} ${size.h / view.scale}` : '0 0 1 1';
  const hovered = hover ? marks.get(hover.node.id) : undefined;
  const hoverAt = hover && (hover.at ?? (view ? toScreen(view, hover.node.x, hover.node.y) : null));

  return (
    <div
      ref={box}
      data-testid="tree-box"
      className="relative min-h-0 flex-1 cursor-grab touch-none overflow-hidden select-none active:cursor-grabbing"
      onWheel={onWheel}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={() => (drag.current = null)}
      onPointerLeave={() => !drag.current && setHover(null)}
    >
      {view && (
        <svg className="absolute inset-0" width={size.w} height={size.h} viewBox={viewBox} data-testid="tree">
          <Edges paths={paths} px={px} />
          <path d={offNodes} fill={OFF_NODE} stroke={OFF_RING} strokeWidth={px(1)} />
          {offJewels.map((n) => (
            <rect
              key={n.id}
              x={n.x - RADIUS.jewel}
              y={n.y - RADIUS.jewel}
              width={RADIUS.jewel * 2}
              height={RADIUS.jewel * 2}
              transform={`rotate(45 ${n.x} ${n.y})`}
              fill={OFF_NODE}
              stroke={OFF_RING}
              strokeWidth={px(1)}
            />
          ))}
          {stage.nodes.map((m) => {
            const n = byId.get(m.id);
            return n && !n.ascendancy ? <MarkedNode key={m.id} n={n} mark={m.mark} px={px} /> : null;
          })}
          {hover && !hover.node.ascendancy && (
            <circle cx={hover.node.x} cy={hover.node.y} r={RADIUS[hover.node.kind] + px(5)} fill="none" stroke="var(--color-value)" strokeWidth={px(1)} />
          )}
        </svg>
      )}
      {labels.map((l) => (
        <span
          key={l.id}
          aria-hidden="true"
          className={`tree-label pointer-events-none absolute font-caps text-[12px] leading-none whitespace-nowrap ${
            l.mark === 'added' ? 'text-quest' : 'text-plain'
          }`}
          style={{ top: l.y - 6, ...(l.anchor === 'start' ? { left: l.tx } : { right: size.w - l.tx }) }}
        >
          {l.text}
        </span>
      ))}
      {size.h > INSET &&
        stage.ascendancies.map((name, i) => (
          <AscendancyMap
            key={name}
            name={name}
            layout={layout}
            byId={byId}
            marks={marks}
            edgeMarks={edgeMarks}
            left={i * (INSET + 4)}
            top={size.h - INSET}
            hover={hover}
            onHover={setHover}
          />
        ))}
      {hover && hoverAt && (
        <NodeCard hover={hover} stage={stage} mark={hovered?.mark} since={hovered?.since} effect={hovered?.effect} at={hoverAt} box={size} />
      )}
      <div className="absolute right-0 bottom-0 flex gap-1">
        <Button onClick={() => setView(null)} onPointerDown={(e) => e.stopPropagation()}>
          Fit
        </Button>
      </div>
    </div>
  );
}

export function TreeApp(): React.JSX.Element {
  const [stage, setStage] = useState<TreeStage | null | undefined>(undefined);
  const [layout, setLayout] = useState<{ version: string | null; layout: TreeLayout } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback((at: number | null) => {
    getTreeStage(at)
      .then((s) => {
        setError(null);
        setStage(s);
      })
      .catch((e: unknown) => {
        setStage(null);
        setError(String(e));
      });
  }, []);

  useEffect(() => {
    load(null);
    const off = isTauri() ? onTreeOpened(() => load(null)) : null;
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') void closeTree();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      void off?.then((f) => f());
      window.removeEventListener('keydown', onKey);
    };
  }, [load]);

  const version = stage?.version ?? null;
  useEffect(() => {
    if (!stage || (layout && layout.version === version)) return;
    getTreeLayout(version)
      .then((l) => setLayout({ version, layout: l }))
      .catch((e: unknown) => setError(String(e)));
  }, [stage, version, layout]);

  const pickMode = (mode: TreeMode): void => {
    void setTreeMode(mode).then(() => load(stage?.stage ?? null));
  };

  return (
    <div className="flex h-full flex-col border border-edge bg-black/90">
      <header
        data-tauri-drag-region
        className="flex h-8 shrink-0 cursor-move items-center gap-2 border-b border-edge bg-black pr-1 pl-3"
      >
        <span data-tauri-drag-region className="font-caps text-[13px] text-normal">
          <Brand />
        </span>
        <span data-tauri-drag-region className="font-caps text-[13px] text-label">
          Passive tree
        </span>
        <Button
          className="ml-auto grid size-6 cursor-default place-items-center p-0"
          aria-label="Close the passive tree"
          onClick={() => void closeTree()}
        >
          <svg viewBox="0 0 12 12" className="size-3" aria-hidden="true">
            <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" fill="none" stroke="currentColor" strokeWidth="1.4" />
          </svg>
        </Button>
      </header>
      {error ? (
        <p className="px-4 py-4 font-body text-[12px] text-corrupted">{error}</p>
      ) : stage === null ? (
        <p className="px-4 py-4 font-body text-[13px] text-label italic">
          Follow a guide or import your build in the settings to see its tree here.
        </p>
      ) : stage ? (
        <>
          <div className="flex shrink-0 items-center gap-2 px-3 pt-3">
            <Select
              value={String(stage.stage)}
              label="Stage"
              className="min-w-0 flex-1"
              options={stage.stages.map((title, i) => ({ value: String(i), label: title }))}
              onChange={(v) => load(Number(v))}
            />
            <div className="flex shrink-0 gap-1">
              <Choice value={stage.mode} options={modes(stage.canCompare)} onChange={pickMode} />
            </div>
          </div>
          <p className="shrink-0 px-3 pt-2 pb-2 font-caps text-[13px] leading-snug text-plain">
            {summary(stage)}
            <Ascendancy stage={stage} />
          </p>
          {layout?.version === version ? (
            // A new stage or mode opens on its own part of the tree.
            <TreeCanvas key={`${stage.stage}:${stage.mode}`} layout={layout.layout} stage={stage} />
          ) : (
            <div className="flex-1" />
          )}
          <Legend stage={stage} />
        </>
      ) : null}
    </div>
  );
}
