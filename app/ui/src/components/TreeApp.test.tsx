import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import type { Mark, NodeMark, TreeLayout, TreeMode, TreePrefs, TreeStage } from '../ipc/types';
import { TreeApp, markName, versionNote } from './TreeApp';

vi.mock('../ipc/commands', () => ({
  getTreeStage: vi.fn((stage: number | null) => Promise.resolve(stageFor(stage ?? 1))),
  getTreeLayout: vi.fn(() => Promise.resolve(LAYOUT)),
  setTreeMode: vi.fn((m: TreeMode) => {
    mode = m;
    return Promise.resolve();
  }),
  closeTree: vi.fn(() => Promise.resolve()),
  getTreePrefs: vi.fn(() => Promise.resolve(prefs)),
  setTreePrefs: vi.fn(() => Promise.resolve()),
}));

let prefs: TreePrefs = { folded: [], list: false };

const LAYOUT: TreeLayout = {
  nodes: [
    { id: 10, x: 0, y: -162, kind: 'normal', name: 'Strength', stats: ['+10 to Strength'] },
    { id: 11, x: 162, y: 0, kind: 'notable', name: 'Heart of Oak', stats: ['+20 to maximum Life', '5% increased Armour'] },
    { id: 13, x: 0, y: 82, kind: 'jewel', name: 'Jewel Socket', stats: [] },
    {
      id: 20,
      x: 1000,
      y: 0,
      kind: 'keystone',
      name: 'Iron Reflexes',
      stats: ['Converts all Evasion Rating to Armour'],
      reminder: ['(Armour reduces Physical Damage)'],
      flavour: ['Steel yourself.'],
    },
    {
      id: 12,
      x: 400,
      y: -100,
      kind: 'mastery',
      name: 'Life Mastery',
      stats: [],
      mastery: [
        { effect: 9755, stats: ['+50 to maximum Life'] },
        { effect: 4119, stats: ['Regenerate 1% of Life per second'] },
      ],
    },
    { id: 30, x: -8000, y: 8918, kind: 'notable', name: 'Radiant Crusade', stats: ['Summon a Sentinel'], ascendancy: 'Guardian' },
    { id: 31, x: -8000, y: 9000, kind: 'ascendancyStart', name: 'Guardian', stats: [], ascendancy: 'Guardian' },
    { id: 60, x: -1000, y: 13000, kind: 'ascendancyStart', name: 'Farrul Bloodline', stats: [], ascendancy: 'Farrul' },
    { id: 61, x: -1000, y: 12918, kind: 'notable', name: 'Huntleader', stats: ['Warcries have 20% increased Area'], ascendancy: 'Farrul' },
  ],
  edges: [
    { a: 10, b: 11, arc: 162, clockwise: true },
    { a: 11, b: 20, arc: null, clockwise: false },
    { a: 13, b: 10, arc: null, clockwise: false },
    { a: 30, b: 31, arc: null, clockwise: true },
    { a: 60, b: 61, arc: null, clockwise: true },
  ],
  ascendancyNames: { Guardian: 'Guardian', Farrul: 'Farrul Bloodline' },
};

let mode: TreeMode = 'tree';
let canCompare = true;

const nm = (id: number, mark: Mark, since: number, effect: number | null = null): NodeMark => ({
  id,
  mark,
  since,
  effect,
  anointed: false,
  tattoo: null,
});

function stageFor(stage: number): TreeStage {
  const shown: TreeMode = mode === 'character' && !canCompare ? 'tree' : mode;
  const base = {
    stages: ['Level 1-12', 'Level 12-22'],
    stage,
    version: '3_29',
    mode: shown,
    canCompare,
    points: 3,
    notables: 1,
    focus: { minX: 0, minY: -162, maxX: 1000, maxY: 82 },
    ascendancies: stage === 0 ? [] : ['Guardian', 'Farrul'],
    latest: null,
    characterVersion: null,
    missing: 0,
    missingNames: [],
    jewels: [],
    clusterNodes: [],
    clusterEdges: [],
  };
  if (shown === 'tree') {
    return {
      ...base,
      added: 0,
      extra: 0,
      addedNames: [],
      ascendancy: [{ name: 'Radiant Crusade', mark: 'taken' }],
      nodes: [...[10, 11, 20, 30, 60, 61].map((id) => nm(id, 'taken', id === 10 ? 0 : 1)), nm(12, 'taken', 1, 4119)],
      edges: [
        { edge: 0, mark: 'taken' },
        { edge: 1, mark: 'taken' },
      ],
    };
  }
  return {
    ...base,
    added: 2,
    extra: 1,
    addedNames: ['Heart of Oak', 'Iron Reflexes'],
    ascendancy: [{ name: 'Radiant Crusade', mark: 'added' }],
    nodes: [nm(10, 'taken', 0), nm(11, 'added', 1), nm(13, 'extra', 0), nm(20, 'added', 1), nm(30, 'added', 1)],
    edges: [
      { edge: 0, mark: 'added' },
      { edge: 1, mark: 'added' },
      { edge: 2, mark: 'extra' },
    ],
  };
}

const size = (el: unknown, prop: string, value: number): void => {
  Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => value });
  void el;
};

beforeAll(() => {
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe(): void {}
      disconnect(): void {}
    },
  );
  size(null, 'clientWidth', 600);
  size(null, 'clientHeight', 400);
});

afterAll(() => {
  delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientWidth;
  delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientHeight;
});

beforeEach(() => {
  mode = 'tree';
  canCompare = true;
  prefs = { folded: [], list: false };
  vi.clearAllMocks();
  vi.mocked(commands.getTreeStage).mockImplementation((stage) => Promise.resolve(stageFor(stage ?? 1)));
});

const marks = (): Record<string, string | null> =>
  Object.fromEntries(
    [...screen.getByTestId('tree').querySelectorAll('[data-node]')].map((g) => [
      g.getAttribute('data-node'),
      g.getAttribute('data-mark'),
    ]),
  );

// Moves the pointer onto a node of the main tree, from where the fitted view put it.
// Where a node of the main tree is in the box: the layer is drawn past the box's edges, so it
// starts left of and above it.
function nodeOnScreen(id: number): { x: number; y: number } {
  const svg = screen.getByTestId('tree');
  const shape = svg.querySelector(`[data-node="${id}"]`)!.firstElementChild!;
  const n = (a: string): number => Number(shape.getAttribute(a));
  // A jewel socket is a rotated square.
  const [cx, cy] = shape.tagName === 'rect' ? [n('x') + n('width') / 2, n('y') + n('height') / 2] : [n('cx'), n('cy')];
  const [vx, vy, vw] = svg.getAttribute('viewBox')!.split(' ').map(Number);
  const scale = Number(svg.getAttribute('width')) / vw;
  const layer = screen.getByTestId('tree-layer');
  return {
    x: (cx - vx) * scale + parseFloat(layer.style.left),
    y: (cy - vy) * scale + parseFloat(layer.style.top),
  };
}

// How much of the tree the drawn layer spans, in tree units. Zooming in makes it smaller.
const viewWidth = (): number => Number(screen.getByTestId('tree').getAttribute('viewBox')!.split(' ')[2]);

function hoverNode(id: number): void {
  const at = nodeOnScreen(id);
  act(() => {
    fireEvent.pointerMove(screen.getByTestId('tree-box'), { clientX: at.x, clientY: at.y });
  });
}

describe('TreeApp', () => {
  it("shows the stage's tree as PoB does by default", async () => {
    render(<TreeApp />);
    expect(await screen.findByText('3 points, 1 notable.')).toBeInTheDocument();
    await screen.findByTestId('tree');
    expect(screen.getByText('Radiant Crusade')).toBeInTheDocument();
    expect(marks()).toEqual({ '10': 'taken', '11': 'taken', '12': 'taken', '20': 'taken' });
    expect(screen.getByRole('button', { name: 'Tree' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('combobox', { name: 'Stage' })).toHaveTextContent('Level 12-22');
    // Notables and keystones the stage takes carry their names.
    expect(screen.getByText('Heart of Oak')).toBeInTheDocument();
    expect(screen.getByText('Iron Reflexes')).toBeInTheDocument();
    expect(screen.queryByText('To take')).toBeNull();
  });

  it('compares with the character and remembers the mode', async () => {
    render(<TreeApp />);
    await userEvent.click(await screen.findByRole('button', { name: 'Vs. character' }));
    expect(commands.setTreeMode).toHaveBeenCalledWith('character');
    expect(await screen.findByText('2 to take')).toBeInTheDocument();
    await screen.findByTestId('tree');
    expect(screen.getByText(/the guide\s+doesn't take/)).toBeInTheDocument();
    expect(marks()).toEqual({ '10': 'taken', '11': 'added', '13': 'extra', '20': 'added' });
    const legend = screen.getByText('Not in the guide').parentElement!;
    expect(within(legend).getByText('To take')).toBeInTheDocument();
    expect(screen.getByTestId('tree').querySelector('[data-edges="added"]')).not.toBeNull();
  });

  it("can't compare without a character", async () => {
    canCompare = false;
    mode = 'character';
    render(<TreeApp />);
    const vs = await screen.findByRole('button', { name: 'Vs. character' });
    expect(vs).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Tree' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('says what the stage adds in New', async () => {
    mode = 'new';
    render(<TreeApp />);
    expect(await screen.findByText('2 new')).toBeInTheDocument();
    expect(screen.getByText(/since Level 1-12/)).toBeInTheDocument();
    expect(screen.getByText('1 refunded')).toBeInTheDocument();
    expect(screen.getByText('Refunded at this stage')).toBeInTheDocument();
  });

  it("colours a node's card as the game does", async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    hoverNode(20);
    const card = await screen.findByRole('tooltip');
    expect(within(card).getByText('Converts all Evasion Rating to Armour')).toHaveClass('text-magic');
    expect(within(card).getByText('(Armour reduces Physical Damage)')).toHaveClass('text-label');
    expect(within(card).getByText('Steel yourself.')).toHaveClass('text-unique', 'italic');
  });

  it('shows the picked mastery effect on its card', async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    hoverNode(12);
    const card = await screen.findByRole('tooltip');
    expect(card).toHaveTextContent('Life Mastery');
    expect(card).toHaveTextContent('Regenerate 1% of Life per second');
    expect(card).not.toHaveTextContent('+50 to maximum Life');
  });

  it('draws the ascendancy apart, with its own card', async () => {
    mode = 'character';
    render(<TreeApp />);
    // The summary comes before the tree's layout, so wait for the map itself.
    const map = await screen.findByTestId('ascendancy-Guardian');
    expect(screen.getByText('2 to take')).toBeInTheDocument();
    expect(map.querySelector('[data-node="30"]')).toHaveAttribute('data-mark', 'added');
    // The start isn't taken, so it's drawn like any node out of play.
    expect(map.querySelector('[data-node="31"]')).toBeNull();
    expect(screen.getByTestId('tree').querySelector('[data-node="30"]')).toBeNull();
    // Fitted into the 150px square, the notable sits near the top middle.
    const svg = map.querySelector('svg')!;
    const [vx, vy, vw] = svg.getAttribute('viewBox')!.split(' ').map(Number);
    const scale = 150 / vw;
    act(() => {
      fireEvent.pointerMove(map, { clientX: (-8000 - vx) * scale, clientY: (8918 - vy) * scale });
    });
    const card = await screen.findByRole('tooltip');
    expect(card).toHaveTextContent('Radiant Crusade');
    expect(card).toHaveTextContent('Notable · To take');
  });

  it('draws a bloodline next to the ascendancy under the name the game uses', async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    const bloodline = screen.getByTestId('ascendancy-Farrul');
    expect(bloodline).toHaveTextContent('Farrul Bloodline');
    expect(bloodline.querySelector('[data-node="61"]')).toHaveAttribute('data-mark', 'taken');
    // The bloodline stacks above the ascendancy.
    const ascendancy = screen.getByTestId('ascendancy-Guardian');
    expect(parseFloat(bloodline.style.top)).toBeLessThan(parseFloat(ascendancy.style.top));
  });

  it('draws no ascendancy before the stage takes a point there', async () => {
    vi.mocked(commands.getTreeStage).mockImplementation(() =>
      Promise.resolve({ ...stageFor(0), nodes: stageFor(0).nodes.filter((n) => n.id < 30) }),
    );
    render(<TreeApp />);
    await screen.findByTestId('tree');
    expect(screen.queryByTestId('ascendancy-Guardian')).toBeNull();
    expect(screen.queryByTestId('ascendancy-Farrul')).toBeNull();
  });

  it('folds a bloodline to a bar and remembers it', async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    await userEvent.click(screen.getByRole('button', { name: 'Fold Farrul Bloodline' }));
    expect(commands.setTreePrefs).toHaveBeenCalledWith({ folded: ['Farrul'], list: false });
    const bar = screen.getByTestId('ascendancy-Farrul');
    expect(bar.querySelector('svg path[d^="M1 5h8M5"]')).not.toBeNull();
    expect(bar.querySelector('[data-node]')).toBeNull();
    expect(screen.getByRole('button', { name: 'Unfold Farrul Bloodline' })).toHaveAttribute('aria-expanded', 'false');
  });

  it('opens folded the way the player left it', async () => {
    prefs = { folded: ['Guardian'], list: false };
    render(<TreeApp />);
    expect(await screen.findByRole('button', { name: 'Unfold Guardian' })).toBeInTheDocument();
    expect(screen.getByTestId('ascendancy-Farrul').querySelector('[data-node="61"]')).not.toBeNull();
  });

  it('lists the keystones, ascendancy notables and masteries next to the tree', async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    expect(screen.queryByRole('complementary')).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'List' }));
    expect(commands.setTreePrefs).toHaveBeenCalledWith({ folded: [], list: true });
    const list = screen.getByRole('complementary', { name: 'Keystones, ascendancy and masteries' });
    const major = [...list.querySelectorAll('[data-list-node]')].map((el) => el.getAttribute('data-list-node'));
    // Ascendancy and bloodline notables first, then the keystones.
    expect(major).toEqual(['30', '61', '20']);
    expect(within(list).getByText('Converts all Evasion Rating to Armour')).toHaveClass('text-magic');
    expect(within(list).getByText('Life Mastery')).toBeInTheDocument();
    expect(within(list).getByText('Regenerate 1% of Life per second')).toBeInTheDocument();
  });

  it('moves the drawn tree while dragging and draws it once at the end', async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    const box = screen.getByTestId('tree-box');
    const layer = screen.getByTestId('tree-layer');
    const before = nodeOnScreen(11);
    const viewBox = screen.getByTestId('tree').getAttribute('viewBox');
    fireEvent.pointerDown(box, { clientX: 100, clientY: 100 });
    fireEvent.pointerMove(box, { clientX: 140, clientY: 90 });
    // Nothing is drawn again while the pointer moves, the layer just follows it.
    expect(layer.style.transform).toContain('translate(40px, -10px)');
    expect(screen.getByTestId('tree').getAttribute('viewBox')).toBe(viewBox);
    act(() => {
      fireEvent.pointerUp(box);
    });
    expect(layer.style.transform).toBe('');
    const after = nodeOnScreen(11);
    expect(after.x - before.x).toBeCloseTo(40);
    expect(after.y - before.y).toBeCloseTo(-10);
  });

  it('zooms with a transform first and draws the zoom a moment later', async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    const box = screen.getByTestId('tree-box');
    const layer = screen.getByTestId('tree-layer');
    const before = viewWidth();
    fireEvent.wheel(box, { deltaY: -200, clientX: 300, clientY: 200 });
    expect(layer.style.transform).toContain('scale(');
    expect(viewWidth()).toBe(before);
    await waitFor(() => expect(layer.style.transform).toBe(''));
    expect(viewWidth()).toBeLessThan(before);
  });

  it('opens a card with the node and the stage that takes it', async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    const tree = screen.getByTestId('tree-box');
    hoverNode(11);
    const card = await screen.findByRole('tooltip');
    expect(card).toHaveTextContent('Heart of Oak');
    expect(card).toHaveTextContent('Notable');
    expect(card).toHaveTextContent('+20 to maximum Life');
    expect(card).toHaveTextContent('The guide takes it at Level 12-22.');
    act(() => {
      fireEvent.pointerMove(tree, { clientX: 2, clientY: 2 });
    });
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('shows jewels with their area, anoints and tattoos', async () => {
    const base = stageFor(1);
    vi.mocked(commands.getTreeStage).mockResolvedValue({
      ...base,
      nodes: [
        ...base.nodes.map((m) => (m.id === 11 ? { ...m, anointed: true } : m.id === 10 ? { ...m, tattoo: { name: 'Tattoo of the Kitava Shaman', stats: ['Attacks have 5% chance to cause Bleeding'] } } : m)),
        nm(13, 'taken', 1),
      ],
      jewels: [
        {
          socket: 13,
          centre: 20,
          radius: 960,
          withoutPath: true,
          item: {
            rarity: 'unique',
            name: 'Impossible Escape',
            base: 'Viridian Jewel',
            quality: null,
            sockets: null,
            levelReq: null,
            radius: 'Small',
            corrupted: true,
            mods: [{ text: 'Passive Skills in radius of Iron Reflexes can be allocated without being connected to your tree', kind: 'explicit' }],
          },
        },
      ],
    });
    render(<TreeApp />);
    await screen.findByTestId('tree');
    const area = screen.getByTestId('jewel-area-13');
    expect(area).toHaveAttribute('r', '960');
    expect(area).toHaveAttribute('stroke-dasharray');

    hoverNode(13);
    const jewel = await screen.findByRole('tooltip');
    expect(within(jewel).getByText('Impossible Escape')).toHaveClass('text-unique');
    expect(jewel).toHaveTextContent('Viridian Jewel');
    expect(jewel).toHaveTextContent('Corrupted');

    hoverNode(11);
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Anointed on the amulet, no point needed.');

    hoverNode(10);
    const tattoo = await screen.findByRole('tooltip');
    expect(tattoo).toHaveTextContent('Tattoo of the Kitava Shaman');
    expect(tattoo).toHaveTextContent('Tattoo on Strength');
    expect(tattoo).toHaveTextContent('Attacks have 5% chance to cause Bleeding');
  });

  it("draws a cluster jewel's passives with the tree and marks the taken ones", async () => {
    const base = stageFor(1);
    const cluster = { id: 65572, x: 400, y: -300, kind: 'notable' as const, name: 'Fuel the Fight', stats: ['20% increased Attack Speed'] };
    vi.mocked(commands.getTreeStage).mockResolvedValue({
      ...base,
      nodes: [...base.nodes, nm(65572, 'taken', 1)],
      edges: [...base.edges, { edge: LAYOUT.edges.length, mark: 'taken' }],
      clusterNodes: [cluster],
      clusterEdges: [{ a: 65572, b: 11, arc: null, clockwise: false }],
    });
    render(<TreeApp />);
    await screen.findByTestId('tree');
    expect(screen.getByTestId('tree').querySelector('[data-node="65572"]')).toHaveAttribute('data-mark', 'taken');
    hoverNode(65572);
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Fuel the Fight');
  });

  it('loads another stage from the select and closes on Esc', async () => {
    render(<TreeApp />);
    await userEvent.click(await screen.findByRole('combobox', { name: 'Stage' }));
    await userEvent.click(screen.getByRole('option', { name: 'Level 1-12' }));
    expect(commands.getTreeStage).toHaveBeenLastCalledWith(0);
    await userEvent.keyboard('{Escape}');
    expect(commands.closeTree).toHaveBeenCalled();
  });

  it('says why there is no tree', async () => {
    vi.mocked(commands.getTreeStage).mockRejectedValueOnce(
      'Path of Building has no 3.30 passive tree yet. Update Path of Building.',
    );
    render(<TreeApp />);
    expect(await screen.findByText(/no 3.30 passive tree yet/)).toBeInTheDocument();
  });

  it('names marks by mode', () => {
    expect(markName('character', 'added')).toBe('To take');
    expect(markName('new', 'added')).toBe('New at this stage');
    expect(markName('new', 'extra')).toBe('Refunded at this stage');
  });

  it('says when the stage is for an older tree and what it lacks of the character', async () => {
    const old = { ...stageFor(1), latest: '3_30' };
    expect(versionNote(stageFor(1))).toBeNull();
    expect(versionNote(old)).toBe('This stage is for the 3.29 tree, Path of Building has 3.30. A few nodes may have moved.');
    const vs = { ...old, mode: 'character' as const, characterVersion: '3_30', missing: 3, missingNames: ['Divine Guidance'] };
    expect(versionNote(vs)).toBe(
      "This stage is for the 3.29 tree, Path of Building has 3.30. 3 of your nodes aren't in the 3.29 tree: Divine Guidance.",
    );
    vi.mocked(commands.getTreeStage).mockResolvedValueOnce(old);
    render(<TreeApp />);
    expect(await screen.findByText(/Path of Building has 3.30/)).toBeInTheDocument();
  });
});
