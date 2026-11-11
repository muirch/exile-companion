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
  ],
  edges: [
    { a: 10, b: 11, arc: 162, clockwise: true },
    { a: 11, b: 20, arc: null, clockwise: false },
    { a: 13, b: 10, arc: null, clockwise: false },
  ],
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
  };
  if (shown === 'tree') {
    return {
      ...base,
      added: 0,
      extra: 0,
      addedNames: [],
      ascendancy: [{ name: 'Radiant Crusade', mark: 'taken' }],
      nodes: [10, 11, 20].map((id) => ({ id, mark: 'taken' as const, since: id === 10 ? 0 : 1 })),
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
      { id: 10, mark: 'taken', since: 0 },
      { id: 11, mark: 'added', since: 1 },
      { id: 13, mark: 'extra', since: 0 },
      { id: 20, mark: 'added', since: 1 },
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
});

const marks = (): Record<string, string | null> =>
  Object.fromEntries(
    [...screen.getByTestId('tree').querySelectorAll('[data-node]')].map((g) => [
      g.getAttribute('data-node'),
      g.getAttribute('data-mark'),
    ]),
  );

describe('TreeApp', () => {
  it("shows the stage's tree as PoB does by default", async () => {
    render(<TreeApp />);
    expect(await screen.findByText('3 points, 1 notable.')).toBeInTheDocument();
    await screen.findByTestId('tree');
    expect(screen.getByText('Radiant Crusade')).toBeInTheDocument();
    expect(marks()).toEqual({ '10': 'taken', '11': 'taken', '20': 'taken' });
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

  it('opens a card with the node and the stage that takes it', async () => {
    render(<TreeApp />);
    await screen.findByTestId('tree');
    const tree = screen.getByTestId('tree-box');
    // The view fits 1000 x 244 tree units plus padding into 600 x 400, so find the node's spot.
    const node = screen.getByTestId('tree').querySelector('[data-node="11"] circle')!;
    const svg = screen.getByTestId('tree');
    const [vx, vy, vw] = svg.getAttribute('viewBox')!.split(' ').map(Number);
    const scale = 600 / vw;
    const cx = (Number(node.getAttribute('cx')) - vx) * scale;
    const cy = (Number(node.getAttribute('cy')) - vy) * scale;
    act(() => {
      fireEvent.pointerMove(tree, { clientX: cx, clientY: cy });
    });
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
