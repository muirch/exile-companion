import type { Bounds, NodeKind, TreeEdge, TreeNode } from '../ipc/types';

/** The tree point at the box's top-left corner, and screen pixels per tree unit. */
export interface View {
  x: number;
  y: number;
  scale: number;
}

/** In tree units, so nodes grow with the zoom as in PoB. */
export const RADIUS: Record<NodeKind, number> = {
  normal: 26,
  notable: 40,
  keystone: 58,
  jewel: 38,
  mastery: 30,
  ascendancyStart: 50,
};

const MIN_SCALE = 0.01;
const MAX_SCALE = 0.6;
// Room around the marked nodes, in tree units, so the outermost ones and their labels fit.
const PAD = 420;

export function bounds(nodes: TreeNode[]): Bounds | null {
  if (nodes.length === 0) return null;
  const b = { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity };
  for (const n of nodes) {
    b.minX = Math.min(b.minX, n.x);
    b.minY = Math.min(b.minY, n.y);
    b.maxX = Math.max(b.maxX, n.x);
    b.maxY = Math.max(b.maxY, n.y);
  }
  return b;
}

/** `b` centred in a `w` by `h` box, with `pad` tree units around it. */
export function fit(b: Bounds, w: number, h: number, pad = PAD): View {
  const bw = b.maxX - b.minX + 2 * pad;
  const bh = b.maxY - b.minY + 2 * pad;
  const scale = clampScale(Math.min(w / bw, h / bh));
  const cx = (b.minX + b.maxX) / 2;
  const cy = (b.minY + b.maxY) / 2;
  return { x: cx - w / 2 / scale, y: cy - h / 2 / scale, scale };
}

function clampScale(s: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, s));
}

/** Zooms by `factor` keeping the tree point under (`px`, `py`) where it is. */
export function zoomAt(v: View, factor: number, px: number, py: number): View {
  const scale = clampScale(v.scale * factor);
  const tx = v.x + px / v.scale;
  const ty = v.y + py / v.scale;
  return { x: tx - px / scale, y: ty - py / scale, scale };
}

/**
 * A pan or zoom the tree shows by moving what's already drawn with a CSS transform, so a drag
 * costs no drawing. `dx`, `dy` move it, `k` scales it around (`px`, `py`) in the box.
 */
export interface Pending {
  dx: number;
  dy: number;
  k: number;
  px: number;
  py: number;
}

/**
 * How far past each edge of a `w` by `h` box the tree is drawn, in screen pixels. A drag or a
 * zoom out shows that part at once, so the player never sees empty ground while moving.
 */
export const overscan = (w: number, h: number): number => Math.max(w, h);

/** The part of a zoom by `k` the view can still take, so the transform stops where the zoom will. */
export function clampFactor(v: View, k: number): number {
  return clampScale(v.scale * k) / v.scale;
}

/** The CSS transform for `p`, on a layer that starts `pad` pixels left of and above the box. */
export function pendingTransform(p: Pending, pad: number): string {
  const ox = p.px + pad;
  const oy = p.py + pad;
  return `translate(${p.dx}px, ${p.dy}px) translate(${ox}px, ${oy}px) scale(${p.k}) translate(${-ox}px, ${-oy}px)`;
}

/** Where the view lands once `p` is drawn for real. */
export function applyPending(v: View, p: Pending): View {
  const zoomed = p.k === 1 ? v : zoomAt(v, p.k, p.px, p.py);
  return p.dx || p.dy ? panBy(zoomed, p.dx, p.dy) : zoomed;
}

/** Whether `p` is close to showing the edge of what's drawn ahead, so it's time to draw again. */
export function outrunsOverscan(p: Pending, w: number, h: number, pad: number): boolean {
  if (Math.abs(p.dx) > pad * 0.6 || Math.abs(p.dy) > pad * 0.6) return true;
  // Zoomed out by k, the box shows 1 / k as much of the layer, most of it on the far side of
  // the pointer at worst.
  return p.k < 1 && Math.max(w, h) * (1 / p.k - 1) > pad * 0.9;
}

export function panBy(v: View, dx: number, dy: number): View {
  return { ...v, x: v.x - dx / v.scale, y: v.y - dy / v.scale };
}

export function toScreen(v: View, x: number, y: number): { x: number; y: number } {
  return { x: (x - v.x) * v.scale, y: (y - v.y) * v.scale };
}

/** The node under the pointer, or the nearest one within a few pixels of it. */
export function nodeAt(nodes: TreeNode[], v: View, px: number, py: number): TreeNode | null {
  const tx = v.x + px / v.scale;
  const ty = v.y + py / v.scale;
  const slop = 4 / v.scale;
  let best: TreeNode | null = null;
  let bestD = Infinity;
  for (const n of nodes) {
    const d = Math.hypot(n.x - tx, n.y - ty);
    if (d <= RADIUS[n.kind] + slop && d < bestD) {
      best = n;
      bestD = d;
    }
  }
  return best;
}

/** SVG path data for an edge, in tree units. */
export function edgePath(a: TreeNode, b: TreeNode, e: TreeEdge): string {
  if (e.arc == null) return `M${a.x} ${a.y}L${b.x} ${b.y}`;
  return `M${a.x} ${a.y}A${e.arc} ${e.arc} 0 0 ${e.clockwise ? 1 : 0} ${b.x} ${b.y}`;
}

/** Circles for many nodes in one path, for the ones drawn alike. */
export function circlesPath(nodes: TreeNode[]): string {
  return nodes
    .map((n) => {
      const r = RADIUS[n.kind];
      return `M${n.x - r} ${n.y}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0`;
    })
    .join('');
}

export interface Label {
  id: number;
  text: string;
  /** Screen position of the node and its radius on screen. */
  x: number;
  y: number;
  r: number;
}

/** Where the text starts or ends, and which side of the node it's on. */
export type Placed<L> = L & { tx: number; anchor: 'start' | 'end' };

const CHAR_WIDTH = 6.4;
const LINE = 14;

/**
 * Labels to the right of their node, or to the left near the box's right edge. A label that would
 * cover one already placed or run out of the box is left out, the node's card still names it.
 */
export function placeLabels<L extends Label>(labels: L[], w: number, h: number): Placed<L>[] {
  const taken: { x0: number; x1: number; y0: number; y1: number }[] = [];
  const out: Placed<L>[] = [];
  for (const l of labels) {
    const width = l.text.length * CHAR_WIDTH;
    const right = l.x + l.r + 5 + width <= w;
    const x0 = right ? l.x + l.r + 5 : l.x - l.r - 5 - width;
    const box = { x0, x1: x0 + width, y0: l.y - LINE / 2, y1: l.y + LINE / 2 };
    const inside = box.x0 >= 0 && box.y0 >= 0 && box.y1 <= h;
    const clash = taken.some((t) => box.x0 < t.x1 && box.x1 > t.x0 && box.y0 < t.y1 && box.y1 > t.y0);
    if (!inside || clash) continue;
    taken.push(box);
    out.push({ ...l, tx: right ? box.x0 : box.x1, anchor: right ? 'start' : 'end' });
  }
  return out;
}
