import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import type { TreeLayout, TreeMode, TreeStage } from '../ipc/types';
import { TreeApp, markName } from './TreeApp';

vi.mock('../ipc/commands', () => ({
  getTreeStage: vi.fn((stage: number | null) => Promise.resolve(stageFor(stage ?? 1))),
  getTreeLayout: vi.fn(() => Promise.resolve(LAYOUT)),
  setTreeMode: vi.fn((m: TreeMode) => {
    mode = m;
    return Promise.resolve();
  }),
  closeTree: vi.fn(() => Promise.resolve()),
}));

const LAYOUT: TreeLayout = {
  nodes: [
    { id: 10, x: 0, y: -162, kind: 'normal', name: 'Strength', stats: ['+10 to Strength'] },
    { id: 11, x: 162, y: 0, kind: 'notable', name: 'Heart of Oak', stats: ['+20 to maximum Life', '5% increased Armour'] },
    { id: 13, x: 0, y: 82, kind: 'jewel', name: 'Jewel Socket', stats: [] },
    { id: 20, x: 1000, y: 0, kind: 'keystone', name: 'Iron Reflexes', stats: ['Converts all Evasion Rating to Armour'] },
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
  };
  if (shown === 'tree') {
    return {
      ...base,
      added: 0,
      extra: 0,
      addedNames: [],
      ascendancy: [{ name: 'Radiant Crusade', mark: 'taken' }],
      nodes: [
        ...[10, 11, 20, 30, 60, 61].map((id) => ({ id, mark: 'taken' as const, since: id === 10 ? 0 : 1, effect: null })),
        { id: 12, mark: 'taken', since: 1, effect: 4119 },
      ],
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
    nodes: [
      { id: 10, mark: 'taken', since: 0, effect: null },
      { id: 11, mark: 'added', since: 1, effect: null },
      { id: 13, mark: 'extra', since: 0, effect: null },
      { id: 20, mark: 'added', since: 1, effect: null },
      { id: 30, mark: 'added', since: 1, effect: null },
    ],
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
function hoverNode(id: number): void {
  const svg = screen.getByTestId('tree');
  const node = svg.querySelector(`[data-node="${id}"] circle`)!;
  const [vx, vy, vw] = svg.getAttribute('viewBox')!.split(' ').map(Number);
  const scale = 600 / vw;
  act(() => {
    fireEvent.pointerMove(screen.getByTestId('tree-box'), {
      clientX: (Number(node.getAttribute('cx')) - vx) * scale,
      clientY: (Number(node.getAttribute('cy')) - vy) * scale,
    });
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
    await screen.findByText('2 to take');
    const map = screen.getByTestId('ascendancy-Guardian');
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
});
