// The browser mock's passive tree: part of PoB's 3.29 tree and the first stages of a Guardian
// guide, written by `cargo run -p exile-core --example dump_tree`. Loaded only when the tree
// window asks, so the overlay's bundle doesn't carry it.

import type { TreeLayout, TreeMode, TreePrefs, TreeStage } from './types';
import MOCK_TREE from './mock-tree.json';

const DATA = MOCK_TREE as unknown as { layout: TreeLayout; stages: TreeStage[] };
const params = new URLSearchParams(window.location.search);

let mode: TreeMode = (params.get('mode') as TreeMode | null) ?? 'tree';
let shown = false;
let prefs: TreePrefs = { folded: params.get('folded')?.split(',') ?? [], list: params.has('list') };

export const treeLayoutMock = (): TreeLayout => DATA.layout;

export function treeStageMock(stage: number | null): TreeStage {
  const at = stage ?? Number(params.get('stage') ?? 2);
  const noCharacter = params.has('nochar');
  const shownMode = noCharacter && mode === 'character' ? 'tree' : mode;
  const ofMode = DATA.stages.filter((s) => s.mode === shownMode);
  // The mock has a few stages only. Another one shows the nearest it has.
  const found = ofMode.find((s) => s.stage === at) ?? ofMode.filter((s) => s.stage <= at).at(-1) ?? ofMode[0];
  return { ...found, canCompare: !noCharacter };
}

export function setTreeModeMock(m: TreeMode): void {
  mode = m;
}

export const treePrefsMock = (): TreePrefs => prefs;

export function setTreePrefsMock(p: TreePrefs): void {
  prefs = p;
}

export const treeShownMock = (): boolean => shown;

export function setTreeShownMock(on: boolean): void {
  shown = on;
}
