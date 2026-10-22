import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import type { RouteView } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';
import { OverlayRoot, screenZoom } from './OverlayRoot';

vi.mock('../ipc/commands', () => ({
  fitOverlay: vi.fn(() => Promise.resolve()),
  routeNext: vi.fn(() => Promise.resolve(null)),
  routePrev: vi.fn(() => Promise.resolve(null)),
}));

const ROUTE: RouteView = {
  section: 'Act 1',
  edge: 30,
  edge_count: 233,
  section_edge: 2,
  section_edge_count: 40,
  area_id: '1_1_town',
  here: [{ parts: [{ kind: 'quest', id: 'a1q1', name: 'Enemy at the Gate' }], sub_steps: [] }],
  then: { parts: [{ kind: 'enter', id: '1_1_3', name: 'The Coast' }], sub_steps: [] },
};

// Most cases look at the full layout; compact ones switch it on themselves.
const initial = { ...useAppStore.getState(), ui: { ...useAppStore.getState().ui, compact: false } };
const ui = (patch: Partial<typeof initial.ui>): void =>
  act(() => useAppStore.setState((s) => ({ ui: { ...s.ui, ...patch } })));

// The box that carries opacity and zoom; the tooltip header inside it has a style of its own.
const routeBox = (): HTMLElement =>
  screen.getByText('Waiting for the game').closest('[style*="zoom"]') as HTMLElement;

describe('screenZoom', () => {
  it('follows the monitor height like the game UI does', () => {
    expect(screenZoom(1080)).toBe(1);
    expect(screenZoom(1440)).toBeCloseTo(1.333, 3);
    // 4K at 150% Windows scaling reports 1440 CSS pixels.
    expect(screenZoom(2160 / 1.5)).toBeCloseTo(1.333, 3);
    expect(screenZoom(600)).toBe(0.75);
  });
});

describe('OverlayRoot', () => {
  beforeAll(() => {
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe(): void {}
        disconnect(): void {}
      },
    );
  });

  beforeEach(() => useAppStore.setState(initial, true));

  // Gem steps come with the route; the old per-act claim list ignored timing and the gem setting.
  it('shows no gem claim from the old guidance', () => {
    act(() =>
      useAppStore.setState({
        route: ROUTE,
        guidance: {
          kind: 'quest_reward',
          headline: 'Claim Arrow Nova Support from Nessa (+9 more)',
          detail: null,
        },
      }),
    );
    render(<OverlayRoot />);
    expect(screen.queryByText(/Claim Arrow Nova/)).not.toBeInTheDocument();
  });

  it('sets the name in the lower-case face, not small caps', () => {
    render(<OverlayRoot />);
    expect(screen.getByText('poebo')).toHaveClass('font-body');
  });

  it('starts with the notice and no route', () => {
    render(<OverlayRoot />);
    expect(screen.getByRole('status')).toHaveTextContent('Hold Tilde in the game for the menu.');
    expect(routeBox()).not.toBeVisible();
  });

  it('swaps the notice for the route once the game is focused', () => {
    render(<OverlayRoot />);
    ui({ gameFocused: true });
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(routeBox()).toBeVisible();
    expect(routeBox()).toHaveClass('opacity-100');
  });

  it('hides the route when the game loses focus, without bringing the notice back', () => {
    render(<OverlayRoot />);
    ui({ gameFocused: true });
    ui({ gameFocused: false });
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(routeBox()).toHaveClass('opacity-0');
  });

  it('stays hidden in the game when the route is turned off', () => {
    useAppStore.setState({ routeEnabled: false });
    render(<OverlayRoot />);
    ui({ gameFocused: true });
    expect(screen.queryByText('Waiting for the game')).not.toBeInTheDocument();
  });

  it('shows the move bar and route buttons while grabbed', async () => {
    useAppStore.setState({ route: ROUTE });
    render(<OverlayRoot />);
    ui({ grabbed: true });

    expect(screen.getByText(/Drag to move, Alt\+M to lock\./)).toBeInTheDocument();
    expect(screen.getByText('Alt+Left / Alt+Right')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(commands.routeNext).toHaveBeenCalled();
  });

  // The window keeps its bottom edge, so nothing may sit below the route while grabbed.
  it('puts the move controls above the route', () => {
    useAppStore.setState({ route: ROUTE });
    render(<OverlayRoot />);
    ui({ grabbed: true });
    const next = screen.getByRole('button', { name: 'Next' });
    const then = screen.getByText('The Coast');
    expect(next.compareDocumentPosition(then) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('drops the header and properties in compact mode', () => {
    useAppStore.setState((s) => ({
      route: ROUTE,
      campaign: {
        ...s.campaign,
        level: 4,
        act: 1,
        zone: { id: '1_1_town', name: 'Lioneye’s Watch', act: 1 },
      },
    }));
    ui({ compact: true, gameFocused: true });
    render(<OverlayRoot />);
    expect(screen.queryByRole('heading')).not.toBeInTheDocument();
    expect(screen.queryByText(/level/)).not.toBeInTheDocument();
    expect(screen.getByText('Lioneye’s Watch · Act 1 · 3 / 40')).toBeInTheDocument();
    expect(screen.getByText('Enemy at the Gate')).toBeInTheDocument();
    expect(screen.getByText('Then:')).toBeInTheDocument();
  });

  describe('fade out', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it('hides the route after the set time and brings it back on the next step', () => {
      useAppStore.setState({ route: ROUTE });
      ui({ fadeAfter: 5, gameFocused: true });
      render(<OverlayRoot />);
      expect(routeBox()).toHaveClass('opacity-100');

      act(() => vi.advanceTimersByTime(5000));
      expect(routeBox()).toHaveClass('opacity-0');

      act(() => useAppStore.setState({ route: { ...ROUTE, edge: ROUTE.edge + 1 } }));
      expect(routeBox()).toHaveClass('opacity-100');
    });

    it('shows the route again after an Alt-Tab away and back', () => {
      useAppStore.setState({ route: ROUTE });
      ui({ fadeAfter: 5, gameFocused: true });
      render(<OverlayRoot />);
      act(() => vi.advanceTimersByTime(5000));
      expect(routeBox()).toHaveClass('opacity-0');

      ui({ gameFocused: false });
      ui({ gameFocused: true });
      expect(routeBox()).toHaveClass('opacity-100');
    });

    it('comes back on a tap of the menu key', () => {
      useAppStore.setState({ route: ROUTE });
      ui({ fadeAfter: 5, gameFocused: true });
      render(<OverlayRoot />);
      act(() => vi.advanceTimersByTime(5000));
      expect(routeBox()).toHaveClass('opacity-0');

      act(() => useAppStore.getState().peek());
      expect(routeBox()).toHaveClass('opacity-100');
      act(() => vi.advanceTimersByTime(5000));
      expect(routeBox()).toHaveClass('opacity-0');
    });

    it('never fades while the overlay is unlocked', () => {
      useAppStore.setState({ route: ROUTE });
      ui({ fadeAfter: 5, gameFocused: true, grabbed: true });
      render(<OverlayRoot />);
      act(() => vi.advanceTimersByTime(60000));
      expect(routeBox()).toHaveClass('opacity-100');
    });
  });

  it('says when the player is off the route', () => {
    useAppStore.setState((s) => ({
      route: ROUTE,
      campaign: { ...s.campaign, zone: { id: 'HideoutFelled', name: 'Felled Hideout', act: 0 } },
    }));
    ui({ gameFocused: true });
    render(<OverlayRoot />);
    expect(screen.getByText('Off route, showing your last step')).toBeInTheDocument();
  });

  it('stays quiet about the route when the zone matches the step', () => {
    useAppStore.setState((s) => ({
      route: ROUTE,
      campaign: { ...s.campaign, zone: { id: '1_1_town', name: 'Lioneye’s Watch', act: 1 } },
    }));
    ui({ gameFocused: true });
    render(<OverlayRoot />);
    expect(screen.queryByText(/Off route/)).not.toBeInTheDocument();
  });

  it('teaches the step keys and the full layout on start', () => {
    ui({ compact: true });
    render(<OverlayRoot />);
    expect(screen.getByRole('status')).toHaveTextContent(
      'Alt+Left and Alt+Right step the route if it gets ahead or behind.',
    );
    expect(screen.getByRole('status')).toHaveTextContent('Turn on Full layout in the menu');
  });

  it('reports its size to the window', () => {
    render(<OverlayRoot />);
    expect(commands.fitOverlay).toHaveBeenCalled();
  });
});

describe('StatusPanel in the overlay', () => {
  beforeEach(() => {
    useAppStore.setState(initial, true);
    ui({ gameFocused: true });
  });

  it('points to the settings when Client.txt is missing', () => {
    render(<OverlayRoot />);
    expect(screen.getByText(/Client\.txt not found\. Hold Tilde in the game/)).toBeInTheDocument();
  });

  it('shows the zone, act, level and step with the here and then parts', () => {
    useAppStore.setState({
      route: ROUTE,
      source: { mode: 'live', path: 'Client.txt', error: null },
      campaign: {
        ...initial.campaign,
        class: 'Witch',
        level: 4,
        act: 1,
        zone: { id: '1_1_town', name: 'Lioneye’s Watch', act: 1 },
        location_kind: 'town',
      },
    });
    render(<OverlayRoot />);

    expect(screen.getByRole('heading', { name: 'Lioneye’s Watch' })).toBeInTheDocument();
    expect(screen.getByText('Act 1 town')).toBeInTheDocument();
    expect(screen.getByText('Witch level')).toBeInTheDocument();
    expect(screen.getByText('3 / 40')).toBeInTheDocument();
    expect(screen.getByText('Enemy at the Gate')).toBeInTheDocument();
    expect(screen.getByText('Then:')).toBeInTheDocument();
    expect(screen.queryByText(/Client\.txt not found/)).not.toBeInTheDocument();
  });

  it('shows the way out alone in a pass-through area', () => {
    useAppStore.setState({ route: { ...ROUTE, here: [] } });
    render(<OverlayRoot />);
    expect(screen.getByText('Nothing to do here')).toBeInTheDocument();
    expect(screen.getByText('The Coast')).toBeInTheDocument();
    expect(screen.queryByText('Then:')).not.toBeInTheDocument();
  });
});
