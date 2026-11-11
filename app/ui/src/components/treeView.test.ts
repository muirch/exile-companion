import { describe, expect, it } from 'vitest';

import type { TreeNode } from '../ipc/types';
import { bounds, edgePath, fit, nodeAt, panBy, placeLabels, toScreen, zoomAt } from './treeView';

const node = (id: number, x: number, y: number, kind: TreeNode['kind'] = 'normal'): TreeNode => ({
  id,
  x,
  y,
  kind,
  name: `Node ${id}`,
  stats: [],
});

describe('tree view', () => {
  it('fits the marked nodes in the middle of the box', () => {
    const v = fit({ minX: 0, minY: 0, maxX: 1160, maxY: 160 }, 400, 200);
    // 2000 x 1000 tree units with the padding, so the width decides.
    expect(v.scale).toBeCloseTo(0.2);
    const centre = toScreen(v, 580, 80);
    expect(centre.x).toBeCloseTo(200);
    expect(centre.y).toBeCloseTo(100);
  });

  it('zooms around the pointer', () => {
    const v = { x: 0, y: 0, scale: 0.1 };
    const z = zoomAt(v, 2, 100, 50);
    expect(z.scale).toBeCloseTo(0.2);
    const p = toScreen(z, 1000, 500);
    expect(p.x).toBeCloseTo(100);
    expect(p.y).toBeCloseTo(50);
    expect(zoomAt(v, 1000, 0, 0).scale).toBe(0.6);
  });

  it('moves the tree with the pointer', () => {
    const v = panBy({ x: 0, y: 0, scale: 0.5 }, 10, -20);
    expect(toScreen(v, 0, 0)).toEqual({ x: 10, y: -20 });
  });

  it('finds the node under the pointer, the nearest when two are close', () => {
    const nodes = [node(1, 0, 0), node(2, 60, 0, 'notable'), node(3, 1000, 0)];
    const v = { x: -100, y: -100, scale: 1 };
    expect(nodeAt(nodes, v, 100, 100)?.id).toBe(1);
    expect(nodeAt(nodes, v, 150, 100)?.id).toBe(2);
    expect(nodeAt(nodes, v, 600, 100)).toBeNull();
  });

  it('draws an orbit edge as an arc', () => {
    const a = node(1, 0, -162);
    const b = node(2, 162, 0);
    expect(edgePath(a, b, { a: 1, b: 2, arc: 162, clockwise: true })).toBe('M0 -162A162 162 0 0 1 162 0');
    expect(edgePath(a, b, { a: 1, b: 2, arc: null, clockwise: false })).toBe('M0 -162L162 0');
  });

  it('leaves out a label that would cover another or run out of the box', () => {
    const placed = placeLabels(
      [
        { id: 1, text: 'Holy Dominion', x: 20, y: 50, r: 4 },
        { id: 2, text: 'Precision', x: 40, y: 52, r: 4 },
        { id: 3, text: 'Sanctity', x: 290, y: 100, r: 4 },
        { id: 4, text: 'Edge', x: 20, y: 2, r: 4 },
      ],
      300,
      200,
    );
    expect(placed.map((p) => [p.id, p.anchor])).toEqual([
      [1, 'start'],
      [3, 'end'],
    ]);
  });

  it('bounds the whole tree when nothing is marked', () => {
    expect(bounds([node(1, -5, 3), node(2, 7, -9)])).toEqual({ minX: -5, minY: -9, maxX: 7, maxY: 3 });
    expect(bounds([])).toBeNull();
  });
});
