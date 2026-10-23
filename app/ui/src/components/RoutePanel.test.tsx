import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';

import type { Part, RouteView, Step } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';
import { ActDone, HereSteps, PassThrough, ThenStep, hasActions } from './RoutePanel';

const step = (...parts: Part[]): Step => ({ parts, sub_steps: [] });

function route(here: Step[], then: Step | null = null): RouteView {
  return {
    section: 'Act 1',
    edge: 0,
    edge_count: 10,
    section_edge: 0,
    section_edge_count: 10,
    finished_section: null,
    area_id: '1_1_2',
    here,
    then,
  };
}

const initial = useAppStore.getState();

describe('RoutePanel', () => {
  beforeEach(() => useAppStore.setState(initial, true));

  it('spaces a count before quest text', () => {
    const r = route([
      step({ kind: 'text', text: 'Find 3x' }, { kind: 'quest_text', text: 'Medicine Chest' }),
    ]);
    useAppStore.setState({ route: r });
    render(<HereSteps />);
    expect(screen.getByRole('listitem')).toHaveTextContent('Find 3x Medicine Chest');
  });

  it('draws the go mark instead of the arrow glyph', () => {
    useAppStore.setState({ route: route([step({ kind: 'text', text: 'Kill boss ➞ take the exit' })]) });
    render(<HereSteps />);
    const item = screen.getByRole('listitem');
    expect(item).toHaveTextContent('Kill boss take the exit');
    expect(item.textContent).not.toContain('➞');
    expect(item.querySelectorAll('svg')).toHaveLength(1);
  });

  it('shows hints under their step', () => {
    useAppStore.setState({
      route: route([
        { parts: [{ kind: 'kill', name: 'Hillock' }], sub_steps: [[{ kind: 'text', text: 'He hits hard' }]] },
      ]),
    });
    render(<HereSteps />);
    expect(screen.getByText('Hillock')).toBeInTheDocument();
    expect(screen.getByText('He hits hard').closest('ul')).toHaveClass('italic');
  });

  it('says so when the route did not load', () => {
    render(<HereSteps />);
    expect(screen.getByText(/The leveling route didn't load\. Restart Exile Companion/)).toBeInTheDocument();
  });

  it('marks the act the player just finished', () => {
    useAppStore.setState({ route: { ...route([]), finished_section: 'Act 1' } });
    render(<ActDone />);
    expect(screen.getByText('Act 1 done')).toHaveClass('text-quest');
  });

  it('ends the campaign instead of a Then line on the last step', () => {
    useAppStore.setState({
      route: { ...route([step({ kind: 'kill', name: 'Kitava' })]), edge: 9, edge_count: 10 },
    });
    render(<ThenStep />);
    expect(screen.getByText("That's the campaign. On to the maps.")).toBeInTheDocument();
  });

  it('drops a leading arrow after Then', () => {
    const then = {
      parts: [
        { kind: 'text' as const, text: '➞ ' },
        { kind: 'enter' as const, id: '1_1_2', name: 'The Coast' },
      ],
      sub_steps: [],
    };
    useAppStore.setState({ route: route([step({ kind: 'kill', name: 'Hillock' })], then) });
    const { container } = render(<ThenStep />);
    expect(container.querySelector('svg')).toBeNull();
    expect(screen.getByText('The Coast')).toBeInTheDocument();
  });

  it('shows the step that leaves the area after Then', () => {
    const then = step({ kind: 'enter', id: '1_1_3', name: 'The Coast' });
    useAppStore.setState({ route: route([step({ kind: 'kill', name: 'Hillock' })], then) });
    render(<ThenStep />);
    expect(screen.getByText('Then:')).toBeInTheDocument();
    expect(screen.getByText('The Coast')).toBeInTheDocument();
  });

  it('turns the way out into the instruction when there is nothing to do', () => {
    const then = step({ kind: 'enter', id: '1_1_3', name: 'The Coast' });
    useAppStore.setState({ route: route([], then) });
    render(<PassThrough />);
    expect(screen.getByText('Nothing to do here')).toBeInTheDocument();
    expect(screen.getByText('The Coast')).toBeInTheDocument();
    expect(screen.queryByText('Then:')).not.toBeInTheDocument();
  });
});

describe('hasActions', () => {
  it('needs at least one step with parts', () => {
    expect(hasActions(null)).toBe(false);
    expect(hasActions(route([]))).toBe(false);
    expect(hasActions(route([{ parts: [], sub_steps: [[{ kind: 'text', text: 'hint' }]] }]))).toBe(false);
    expect(hasActions(route([step({ kind: 'waypoint' })]))).toBe(true);
  });
});
