// The browser mock's passive tree: part of PoB's 3.29 tree and the first stages of a Guardian
// guide, written by `cargo run -p exile-core --example dump_tree`. Loaded only when the tree
// window asks, so the overlay's bundle doesn't carry it.

import type { TreeLayout, TreeMode, TreeStage } from './types';
import MOCK_TREE from './mock-tree.json';

const DATA = MOCK_TREE as unknown as { layout: TreeLayout; stages: TreeStage[] };
const MODES: TreeMode[] = ['tree', 'new', 'character'];
const params = new URLSearchParams(window.location.search);

let mode: TreeMode = (params.get('mode') as TreeMode | null) ?? 'tree';
let shown = false;

export const treeLayoutMock = (): TreeLayout => DATA.layout;

export function treeStageMock(stage: number | null): TreeStage {
  const stages = DATA.stages.length / MODES.length;
  const at = Math.min(stage ?? 2, stages - 1);
  const noCharacter = params.has('nochar');
  const shownMode = noCharacter && mode === 'character' ? 'tree' : mode;
  const found = DATA.stages[at * MODES.length + MODES.indexOf(shownMode)];
  return { ...found, canCompare: !noCharacter };
}

export function setTreeModeMock(m: TreeMode): void {
  mode = m;
}

export const treeShownMock = (): boolean => shown;

export function setTreeShownMock(on: boolean): void {
  shown = on;
}
