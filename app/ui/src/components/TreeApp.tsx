import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';

import { closeTree, getTreeLayout, getTreePrefs, getTreeStage, setTreeMode, setTreePrefs } from '../ipc/commands';
import { isTauri } from '../ipc/env';
import { onTreeOpened } from '../ipc/events';
import type { JewelMark, Mark, NodeKind, NodeMark, TreeLayout, TreeMode, TreeNode, TreePrefs, TreeStage } from '../ipc/types';
import { Button, Choice, Select } from '../kit';
import { ItemText, RARITY_COLOR } from './ItemText';
import { Brand } from './Tooltip';
import {
  applyPending,
  bounds,
  circlesPath,
  clampFactor,
  edgePath,
  fit,
  nodeAt,
  outrunsOverscan,
  overscan,
  panBy,
  pendingTransform,
  placeLabels,
  RADIUS,
  toScreen,
  zoomAt,
  type Pending,
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

const dotted = (v: string): string => v.replace('_', '.');

const clustered = new WeakMap<TreeStage, { from: TreeLayout; out: TreeLayout }>();

/**
 * The layout with the stage's cluster jewel passives. The same object for the same stage and
 * layout, so the canvas doesn't redraw on every render.
 */
export function withClusters(layout: TreeLayout, stage: TreeStage): TreeLayout {
  if (stage.clusterNodes.length === 0) return layout;
  const hit = clustered.get(stage);
  if (hit?.from === layout) return hit.out;
  const out = { ...layout, nodes: [...layout.nodes, ...stage.clusterNodes], edges: [...layout.edges, ...stage.clusterEdges] };
  clustered.set(stage, { from: layout, out });
  return out;
}

/** Says when the stage's tree is older than Path of Building's, and what of it this tree lacks. */
export function versionNote(s: TreeStage): string | null {
  if (!s.version || (!s.latest && s.missing === 0)) return null;
  const tree = `the ${dotted(s.version)} tree`;
  const whose = s.mode === 'character' && s.characterVersion ? 'of your nodes' : 'nodes of this stage';
  const names = s.missingNames.length ? `: ${s.missingNames.join(', ')}` : '';
  const lacks = s.missing > 0 ? `${s.missing} ${whose} aren't in ${tree}${names}.` : null;
  if (!s.latest) return lacks;
  const older = `This stage is for ${tree}, Path of Building has ${dotted(s.latest)}.`;
  return lacks ? `${older} ${lacks}` : `${older} A few nodes may have moved.`;
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

const unscaled = (v: number): number => v;

// Lavender, the game's colour for what's laid onto an item, marks what items lay onto the tree.
const ITEM_MARK = 'var(--color-crafted)';

/** The ink mark on a tattooed node's shoulder: a small turned square. */
function TattooMark({ x, y, r, px }: { x: number; y: number; r: number; px: (v: number) => number }): React.JSX.Element {
  const s = Math.max(r * 0.45, px(3));
  const cx = x + r * 0.71;
  const cy = y - r * 0.71;
  return (
    <rect
      data-tattoo
      x={cx - s}
      y={cy - s}
      width={s * 2}
      height={s * 2}
      transform={`rotate(45 ${cx} ${cy})`}
      fill={ITEM_MARK}
      stroke="#0a0a0a"
      strokeWidth={px(1)}
    />
  );
}

/** A dashed halo: taken by the amulet's anoint, no path to it, like Impossible Escape's dashed area. */
function AnointMark({ x, y, r, px }: { x: number; y: number; r: number; px: (v: number) => number }): React.JSX.Element {
  return (
    <circle
      data-anointed
      cx={x}
      cy={y}
      r={r + px(5)}
      fill="none"
      stroke={ITEM_MARK}
      strokeWidth={px(1.2)}
      strokeDasharray={`${px(3)} ${px(2.5)}`}
    />
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
      {stage.nodes.some((m) => m.tattoo) && (
        <span>
          <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" className="mr-1 inline-block align-[-2px]">
            <circle cx="6" cy="8" r="4" fill={COLOR.taken} />
            <TattooMark x={6} y={8} r={4} px={unscaled} />
          </svg>
          Tattoo
        </span>
      )}
      {stage.nodes.some((m) => m.anointed) && (
        <span>
          <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" className="mr-1 inline-block align-[-3px]">
            <circle cx="8" cy="8" r="3.5" fill={COLOR.taken} />
            <AnointMark x={8} y={8} r={1.8} px={unscaled} />
          </svg>
          Anointed
        </span>
      )}
      <span className="ml-auto">Drag to move, wheel to zoom</span>
    </div>
  );
}

// The browser mock opens a card with ?hover, for screenshots.
function mockHover(layout: TreeLayout, stage: TreeStage): Hover | null {
  const kind = new URLSearchParams(window.location.search).get('hover');
  if (isTauri() || kind === null) return null;
  // ?hover=jewel, anoint or tattoo goes with ?items.
  const item = { jewel: stage.jewels[0]?.socket, anoint: stage.nodes.find((m) => m.anointed)?.id, tattoo: stage.nodes.find((m) => m.tattoo)?.id }[
    kind as 'jewel' | 'anoint' | 'tattoo'
  ];
  if (item != null) {
    const node = layout.nodes.find((n) => n.id === item);
    return node ? { node } : null;
  }
  const want = (n: TreeNode | undefined): boolean =>
    kind === 'mastery' ? n?.kind === 'mastery' : n?.kind === 'notable' && (kind === 'ascendancy') === Boolean(n.ascendancy);
  const marked = stage.nodes.find((m) => (kind === 'mastery' || m.mark === (stage.mode === 'tree' ? 'taken' : 'added')) && want(layout.nodes.find((n) => n.id === m.id)));
  const node = layout.nodes.find((n) => n.id === marked?.id);
  return node ? { node } : null;
}

interface Hover {
  node: TreeNode;
}

// Small bloodlines would fill the map with huge nodes, so no map zooms in further than this.
const INSET_MAX_SCALE = 0.12;

/** The view an ascendancy's corner map is drawn with. */
function insetView(nodes: TreeNode[]): View | null {
  const b = bounds(nodes);
  if (!b) return null;
  const v = fit(b, INSET, INSET, 160);
  if (v.scale <= INSET_MAX_SCALE) return v;
  const scale = INSET_MAX_SCALE;
  return { x: (b.minX + b.maxX) / 2 - INSET / 2 / scale, y: (b.minY + b.maxY) / 2 - INSET / 2 / scale, scale };
}

const FOLDED = 24;

/** The corner maps stacked up from the bottom left of a box `h` tall, a folded one as a bar. */
function insetBoxes(names: string[], folded: string[], h: number): { left: number; top: number; height: number }[] {
  let bottom = h;
  return names.map((name) => {
    const height = folded.includes(name) ? FOLDED : INSET;
    const box = { left: 0, top: bottom - height, height };
    bottom -= height + 4;
    return box;
  });
}

/** A node the stage marks, filled when taken, ringed when added, slashed when extra. */
function MarkedNode({
  n,
  m,
  px,
  grow = 1,
}: {
  n: TreeNode;
  m: NodeMark;
  px: (v: number) => number;
  grow?: number;
}): React.JSX.Element {
  const { mark } = m;
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
      {m.anointed && <AnointMark x={n.x} y={n.y} r={r} px={px} />}
      {m.tattoo && <TattooMark x={n.x} y={n.y} r={r} px={px} />}
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

/** The area each jewel reaches, in its rarity's colour. Dashed when nodes in it need no path. */
function JewelAreas({
  jewels,
  byId,
  px,
}: {
  jewels: JewelMark[];
  byId: Map<number, TreeNode>;
  px: (v: number) => number;
}): React.JSX.Element {
  return (
    <>
      {jewels.map((j) => {
        const c = byId.get(j.centre);
        if (!c || j.radius == null) return null;
        const color = RARITY_COLOR[j.item.rarity];
        return (
          <circle
            key={j.socket}
            data-testid={`jewel-area-${j.socket}`}
            cx={c.x}
            cy={c.y}
            r={j.radius}
            fill={color}
            fillOpacity={0.05}
            stroke={color}
            strokeOpacity={0.55}
            strokeWidth={px(1.5)}
            strokeDasharray={j.withoutPath ? `${px(6)} ${px(4)}` : undefined}
          />
        );
      })}
    </>
  );
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
  folded,
  onFold,
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
  folded: boolean;
  onFold: () => void;
  hover: Hover | null;
  onHover: (h: Hover | null) => void;
}): React.JSX.Element | null {
  const nodes = useMemo(() => layout.nodes.filter((n) => n.ascendancy === name), [layout, name]);
  const paths = useMemo(() => edgePaths(layout, byId, edgeMarks, (n) => n.ascendancy === name), [layout, byId, edgeMarks, name]);
  const v = insetView(nodes);
  if (!v) return null;
  const title = layout.ascendancyNames[name] ?? name;
  const fold = (
    <Button
      className={`grid size-5 shrink-0 cursor-default place-items-center p-0 ${folded ? '' : 'absolute top-0.5 right-0.5'}`}
      aria-label={`${folded ? 'Unfold' : 'Fold'} ${title}`}
      aria-expanded={!folded}
      onClick={onFold}
    >
      <svg viewBox="0 0 10 10" className="size-2.5" aria-hidden="true">
        <path d={folded ? 'M1 5h8M5 1v8' : 'M1 5h8'} fill="none" stroke="currentColor" strokeWidth="1.4" />
      </svg>
    </Button>
  );
  if (folded) {
    return (
      // Folded, it covers only as much of the tree as its name needs.
      <div
        data-testid={`ascendancy-${name}`}
        className="absolute flex cursor-default items-center gap-1.5 border border-edge bg-[#050505] pr-px pl-1.5"
        style={{ left, top, maxWidth: INSET, height: FOLDED }}
        onPointerDown={(e) => e.stopPropagation()}
        onPointerMove={(e) => e.stopPropagation()}
      >
        <span className="truncate font-caps text-[11px] text-label">{title}</span>
        {fold}
      </div>
    );
  }
  const px = (x: number): number => x / v.scale;
  const onMove = (e: React.PointerEvent<HTMLDivElement>): void => {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    const n = nodeAt(nodes, v, e.clientX - r.left, e.clientY - r.top);
    if (!n) return onHover(null);
    if (n.id !== hover?.node.id) onHover({ node: n });
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
          if (m) return <MarkedNode key={n.id} n={n} m={m} px={px} grow={INSET_GROW} />;
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
      <span className="pointer-events-none absolute top-1 left-1.5 font-caps text-[11px] leading-tight text-label">{title}</span>
      {fold}
    </div>
  );
}

function NodeCard({
  hover,
  stage,
  m,
  at,
  box,
}: {
  hover: Hover;
  stage: TreeStage;
  m: NodeMark | undefined;
  at: { x: number; y: number };
  box: { w: number; h: number };
}): React.JSX.Element {
  const { node } = hover;
  const mark = m?.mark;
  const since = m?.since;
  const tattoo = m?.tattoo;
  const jewel = stage.jewels.find((j) => j.socket === node.id);
  const stats = tattoo?.stats ?? node.mastery?.find((e) => e.effect === m?.effect)?.stats ?? node.stats;
  const r = 10;
  const left = at.x + r + CARD_WIDTH > box.w ? Math.max(0, at.x - r - CARD_WIDTH) : at.x + r;
  const below = at.y < box.h / 2;
  const kind = [
    tattoo ? `Tattoo on ${node.name}` : KIND[node.kind],
    mark && stage.mode !== 'tree' && mark !== 'taken' ? markName(stage.mode, mark) : null,
  ]
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
      <p className="font-caps text-[15px] leading-snug text-value">{tattoo?.name ?? node.name}</p>
      <p className="font-caps text-[13px] text-label">{kind}</p>
      {jewel && (
        <div className="mt-1">
          <ItemText item={jewel.item} />
        </div>
      )}
      {stats.length > 0 && (
        <p className="mt-1 font-body text-[13px] leading-snug whitespace-pre-line text-magic">{stats.join('\n')}</p>
      )}
      {node.reminder?.map((text) => (
        <p key={text} className="font-body text-[12px] leading-snug text-label">
          {text}
        </p>
      ))}
      {node.flavour && node.flavour.length > 0 && (
        <p className="mt-1 font-body text-[13px] leading-snug whitespace-pre-line text-unique italic">{node.flavour.join('\n')}</p>
      )}
      {m?.anointed && <p className="mt-1 font-body text-[13px] leading-snug text-plain italic">Anointed on the amulet, no point needed.</p>}
      {where && !m?.anointed && <p className="mt-1 font-body text-[13px] leading-snug text-plain italic">{where}</p>}
    </div>
  );
}


const LIST_WIDTH = 220;

type Row = { m: TreeStage['nodes'][number]; n: TreeNode };

function ListSection({ title, children }: { title: string; children: React.ReactNode }): React.JSX.Element {
  return (
    <section className="mt-1 first:mt-0">
      <h2 className="py-1 font-caps text-[11px] text-label">{title}</h2>
      <ul className="flex flex-col gap-2">{children}</ul>
    </section>
  );
}

/**
 * What the stage takes besides small passives, in groups the way poe.ninja lists a build: the
 * ascendancy notables and keystones with their stats, then the masteries with the picked effects.
 */
function BuildList({ layout, stage }: { layout: TreeLayout; stage: TreeStage }): React.JSX.Element {
  const byId = new Map(layout.nodes.map((n) => [n.id, n]));
  const rows: Row[] = stage.nodes.flatMap((m) => {
    const n = byId.get(m.id);
    return n ? [{ m, n }] : [];
  });
  const big = rows.filter(({ n }) => n.kind === 'keystone' || (n.ascendancy && n.kind === 'notable'));
  // Ascendancy notables first, in the order the guide takes them, then the keystones.
  const major = [...big.filter((r) => r.n.ascendancy), ...big.filter((r) => !r.n.ascendancy)];
  const masteries = new Map<string, Row[]>();
  for (const r of rows.filter(({ n }) => n.kind === 'mastery')) masteries.set(r.n.name, [...(masteries.get(r.n.name) ?? []), r]);
  const swatch = (m: Row['m']): React.ReactNode => stage.mode !== 'tree' && m.mark !== 'taken' && <Swatch mark={m.mark} />;
  return (
    <aside
      aria-label="Keystones, ascendancy and masteries"
      className="chat-scroll absolute top-0 right-0 bottom-8 overflow-y-auto border-l border-edge bg-[#050505] px-3 py-1"
      style={{ width: LIST_WIDTH }}
      onPointerDown={(e) => e.stopPropagation()}
      onPointerMove={(e) => e.stopPropagation()}
      onWheel={(e) => e.stopPropagation()}
    >
      {major.length > 0 && (
        <ListSection title="Ascendancy & keystones">
          {major.map(({ m, n }) => (
            <li key={n.id} data-list-node={n.id} className="lazy-row [contain-intrinsic-size:auto_70px]">
              <p className="flex items-baseline gap-1 font-caps text-[13px] leading-snug text-value">
                {swatch(m)}
                {n.name}
                <span className="ml-auto font-caps text-[11px] text-label">
                  {n.ascendancy ? (layout.ascendancyNames[n.ascendancy] ?? n.ascendancy) : 'Keystone'}
                </span>
              </p>
              <p className="font-body text-[12px] leading-snug whitespace-pre-line text-magic">{n.stats.join('\n')}</p>
            </li>
          ))}
        </ListSection>
      )}
      <ListSection title="Masteries">
        {masteries.size === 0 && <li className="font-body text-[12px] text-label italic">None at this stage.</li>}
        {[...masteries].map(([name, picks]) => (
          <li key={name} data-list-mastery={name} className="lazy-row [contain-intrinsic-size:auto_50px]">
            <p className="flex items-baseline gap-1 font-caps text-[13px] leading-snug text-value">
              {name}
              {picks.length > 1 && <span className="font-body text-[11px] text-label">×{picks.length}</span>}
            </p>
            {picks.map(({ m, n }) => {
              const effect = n.mastery?.find((e) => e.effect === m.effect);
              return (
                <p key={n.id} className="flex items-baseline gap-1 font-body text-[12px] leading-snug text-plain">
                  {swatch(m)}
                  {effect ? <span className="whitespace-pre-line">{effect.stats.join('\n')}</span> : <span className="text-label italic">No effect picked</span>}
                </p>
              );
            })}
          </li>
        ))}
      </ListSection>
    </aside>
  );
}

function TreeCanvas({
  layout,
  stage,
  prefs,
  onPrefs,
}: {
  layout: TreeLayout;
  stage: TreeStage;
  prefs: TreePrefs;
  onPrefs: (p: TreePrefs) => void;
}): React.JSX.Element {
  const box = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  // Until the player moves the tree it stays fitted, also when the window is resized.
  const [moved, setMoved] = useState<View | null>(null);
  const [hover, setHover] = useState<Hover | null>(() => mockHover(layout, stage));
  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  const layer = useRef<HTMLDivElement>(null);
  // A drag or zoom in progress, shown by moving the drawn layer. It's drawn for real at the end.
  const pending = useRef<Pending | null>(null);
  const zoomTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

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

  // The mastery list covers the right side, so the tree fits into what's left of the box.
  const treeW = Math.max(0, size.w - (prefs.list ? LIST_WIDTH : 0));
  const fitted = useMemo(() => (focus && treeW > 0 ? fit(focus, treeW, size.h) : null), [focus, treeW, size.h]);
  const view = moved ?? fitted;
  const setView = (v: View | null): void => {
    setMoved(v);
    setHover(null);
  };
  const pad = overscan(size.w, size.h);
  // Pointer events can come between a commit and the render that draws it, so they read this.
  const latest = useRef(view);
  latest.current = view;

  // The new view is drawn by now, so the layer can drop the transform that stood in for it.
  useLayoutEffect(() => {
    if (layer.current && !pending.current) layer.current.style.transform = '';
  }, [view]);

  useEffect(() => () => clearTimeout(zoomTimer.current), []);

  const show = (p: Pending): void => {
    pending.current = p;
    if (layer.current) layer.current.style.transform = pendingTransform(p, pad);
    setHover(null);
    if (outrunsOverscan(p, size.w, size.h, pad)) commit();
  };

  function commit(): void {
    clearTimeout(zoomTimer.current);
    const p = pending.current;
    pending.current = null;
    if (!p || !latest.current) return;
    const next = applyPending(latest.current, p);
    latest.current = next;
    setView(next);
  }

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
    const was = pending.current;
    // A zoom keeps its pivot, so moving the pointer between wheel turns starts a new one.
    if (was && (was.dx || was.dy || Math.hypot(was.px - p.x, was.py - p.y) > 2)) commit();
    const base = latest.current ?? view;
    const k = clampFactor(base, (pending.current?.k ?? 1) * Math.pow(1.0015, -e.deltaY));
    show({ dx: 0, dy: 0, k, px: p.x, py: p.y });
    clearTimeout(zoomTimer.current);
    zoomTimer.current = setTimeout(commit, 150);
  };

  const onPointerDown = (e: React.PointerEvent): void => {
    if (pending.current) commit();
    drag.current = { ...point(e), moved: false };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  };

  const onPointerUp = (): void => {
    if (drag.current?.moved) commit();
    drag.current = null;
  };

  const onPointerMove = (e: React.PointerEvent): void => {
    if (!view) return;
    const p = point(e);
    const d = drag.current;
    if (d && (d.moved || Math.hypot(p.x - d.x, p.y - d.y) > 3)) {
      d.moved = true;
      show({ dx: p.x - d.x, dy: p.y - d.y, k: 1, px: 0, py: 0 });
      // Drawn again past the overscan, so the drag goes on from here.
      if (!pending.current) {
        d.x = p.x;
        d.y = p.y;
      }
      return;
    }
    if (pending.current) return;
    const n = nodeAt(hoverable, view, p.x, p.y);
    if (!n) setHover(null);
    else if (n.id !== hover?.node.id) setHover({ node: n });
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
        const at = { x: s.x + pad, y: s.y + pad };
        return { id: m.id, text: n!.name, ...at, r: RADIUS[n!.kind] * view.scale, mark: m.mark, kind: n!.kind };
      });
    // What's still to take and keystones win the room first.
    const rank = (l: (typeof wanted)[number]): number => (l.mark === 'added' ? 0 : 2) + (l.kind === 'keystone' ? 0 : 1);
    // Placed over the overscan too, so labels come along with a drag.
    const placed = placeLabels(
      [0, 1, 2, 3].flatMap((r) => wanted.filter((l) => rank(l) === r)),
      treeW + 2 * pad,
      size.h + 2 * pad,
    );
    return placed;
  }, [view, stage, byId, treeW, size.h, pad]);

  const px = (v: number): number => (view ? v / view.scale : v);
  const layerW = size.w + 2 * pad;
  const layerH = size.h + 2 * pad;
  const viewBox = view
    ? `${view.x - pad / view.scale} ${view.y - pad / view.scale} ${layerW / view.scale} ${layerH / view.scale}`
    : '0 0 1 1';
  const hovered = hover ? marks.get(hover.node.id) : undefined;
  const boxes = insetBoxes(stage.ascendancies, prefs.folded, size.h);
  const fold = (name: string): void => {
    const folded = prefs.folded.includes(name) ? prefs.folded.filter((f) => f !== name) : [...prefs.folded, name];
    onPrefs({ ...prefs, folded });
    setHover(null);
  };
  const hoverAt = ((): { x: number; y: number } | null => {
    if (!hover) return null;
    const asc = hover.node.ascendancy;
    if (!asc) return view ? toScreen(view, hover.node.x, hover.node.y) : null;
    const v = insetView(layout.nodes.filter((n) => n.ascendancy === asc));
    const corner = boxes[stage.ascendancies.indexOf(asc)];
    if (!v || !corner || prefs.folded.includes(asc)) return null;
    const s = toScreen(v, hover.node.x, hover.node.y);
    return { x: corner.left + s.x, y: corner.top + s.y };
  })();

  return (
    <div
      ref={box}
      data-testid="tree-box"
      className="relative min-h-0 flex-1 cursor-grab touch-none overflow-hidden select-none active:cursor-grabbing"
      onWheel={onWheel}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={() => !drag.current && setHover(null)}
    >
      <div
        ref={layer}
        data-testid="tree-layer"
        className="pointer-events-none absolute origin-top-left will-change-transform"
        style={{ left: -pad, top: -pad, width: layerW, height: layerH }}
      >
        {view && (
          <svg className="absolute inset-0" width={layerW} height={layerH} viewBox={viewBox} data-testid="tree">
            <JewelAreas jewels={stage.jewels} byId={byId} px={px} />
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
              return n && !n.ascendancy ? <MarkedNode key={m.id} n={n} m={m} px={px} /> : null;
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
            style={{ top: l.y - 6, ...(l.anchor === 'start' ? { left: l.tx } : { right: layerW - l.tx }) }}
          >
            {l.text}
          </span>
        ))}
      </div>
      {stage.ascendancies.map((name, i) =>
        boxes[i].top < 0 ? null : (
          <AscendancyMap
            key={name}
            name={name}
            layout={layout}
            byId={byId}
            marks={marks}
            edgeMarks={edgeMarks}
            left={boxes[i].left}
            top={boxes[i].top}
            folded={prefs.folded.includes(name)}
            onFold={() => fold(name)}
            hover={hover}
            onHover={setHover}
          />
        ),
      )}
      {hover && hoverAt && (
        <NodeCard hover={hover} stage={stage} m={hovered} at={hoverAt} box={size} />
      )}
      {prefs.list && <BuildList layout={layout} stage={stage} />}
      <div className="absolute right-0 bottom-0 flex gap-1" onPointerDown={(e) => e.stopPropagation()}>
        <Button aria-pressed={prefs.list} onClick={() => onPrefs({ ...prefs, list: !prefs.list })}>
          List
        </Button>
        <Button onClick={() => setView(null)}>Fit</Button>
      </div>
    </div>
  );
}

export function TreeApp(): React.JSX.Element {
  const [stage, setStage] = useState<TreeStage | null | undefined>(undefined);
  const [layout, setLayout] = useState<{ version: string | null; layout: TreeLayout } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [prefs, setPrefs] = useState<TreePrefs>({ folded: [], list: false });

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
    void getTreePrefs().then(setPrefs);
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

  const changePrefs = (p: TreePrefs): void => {
    setPrefs(p);
    void setTreePrefs(p);
  };

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
          {versionNote(stage) && (
            <p className="-mt-1 shrink-0 px-3 pb-2 font-body text-[12px] leading-snug text-plain">
              {versionNote(stage)}
            </p>
          )}
          {layout?.version === version ? (
            // A new stage or mode opens on its own part of the tree.
            <TreeCanvas
              key={`${stage.stage}:${stage.mode}`}
              layout={withClusters(layout.layout, stage)}
              stage={stage}
              prefs={prefs}
              onPrefs={changePrefs}
            />
          ) : (
            <div className="flex-1" />
          )}
          <Legend stage={stage} />
        </>
      ) : null}
    </div>
  );
}
