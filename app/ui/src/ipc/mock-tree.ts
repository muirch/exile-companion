// The browser mock's passive tree: part of PoB's 3.29 tree and the first stages of a Guardian
// guide, written by `cargo run -p exile-core --example dump_tree`. Loaded only when the tree
// window asks, so the overlay's bundle doesn't carry it.

import type { TreeLayout, TreeMode, TreeNode, TreePrefs, TreeStage } from './types';
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
  // ?newer: Path of Building got the next league's tree and the character was imported on it.
  const newer = params.has('newer');
  const compared = newer && shownMode === 'character';
  const versions: Pick<TreeStage, 'latest' | 'characterVersion' | 'missing' | 'missingNames'> = {
    latest: newer ? '3_30' : null,
    characterVersion: compared ? '3_30' : null,
    missing: compared ? 3 : 0,
    missingNames: compared ? ['Divine Guidance'] : [],
  };
  const nodes = found.nodes.map((n) => ({ ...n, anointed: n.anointed ?? false, tattoo: n.tattoo ?? null }));
  const out = { ...found, ...versions, nodes, jewels: found.jewels ?? [], canCompare: !noCharacter };
  return params.has('items') ? withItems(out) : out;
}

// ?items: a jewel, a tattoo and an anoint on the stage, near what it takes, since the mock's
// early stages have none.
function withItems(s: TreeStage): TreeStage {
  const byId = new Map(DATA.layout.nodes.map((n) => [n.id, n]));
  const taken = s.nodes.flatMap((m) => {
    const n = byId.get(m.id);
    return n && !n.ascendancy ? [n] : [];
  });
  const cx = taken.reduce((a, n) => a + n.x, 0) / taken.length;
  const cy = taken.reduce((a, n) => a + n.y, 0) / taken.length;
  const dist = (n: { x: number; y: number }): number => Math.hypot(n.x - cx, n.y - cy);
  const near = (kind: string): number | undefined =>
    DATA.layout.nodes
      .filter((n) => n.kind === kind && !n.ascendancy && !s.nodes.some((m) => m.id === n.id))
      .reduce<TreeNode | undefined>((best, n) => (!best || dist(n) < dist(best) ? n : best), undefined)?.id;
  const socket = near('jewel');
  const anoint = near('notable');
  const tattooOn = taken.find((n) => n.kind === 'normal')?.id;
  const nodes = s.nodes.map((m) =>
    m.id === tattooOn ? { ...m, tattoo: { name: 'Tattoo of the Kitava Shaman', stats: ['Attacks have 5% chance to cause Bleeding'] } } : m,
  );
  if (anoint != null) nodes.push({ id: anoint, mark: 'taken', since: null, effect: null, anointed: true, tattoo: null });
  if (socket != null) nodes.push({ id: socket, mark: 'taken', since: s.stage, effect: null, anointed: false, tattoo: null });
  const jewels: TreeStage['jewels'] =
    socket == null
      ? []
      : [
          {
            socket,
            centre: socket,
            radius: 1800,
            withoutPath: false,
            item: {
              rarity: 'rare',
              name: 'Gloom Hope',
              base: 'Cobalt Jewel',
              quality: null,
              sockets: null,
              levelReq: 40,
              radius: 'Large',
              corrupted: false,
              mods: [
                { text: '+7% to all Elemental Resistances', kind: 'explicit' },
                { text: 'Minions deal 14% increased Damage', kind: 'explicit' },
                { text: '6% increased maximum Life', kind: 'crafted' },
              ],
            },
          },
        ];
  return { ...s, nodes, jewels };
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
