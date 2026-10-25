import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import type { BuildSummary } from '../ipc/types';
import { useAppStore } from '../store/useAppStore';
import { GemRows, overrideFrom, overrideValue } from './GemRows';

vi.mock('../ipc/commands', () => ({
  getGemRows: vi.fn(),
  setGemOverride: vi.fn(),
}));

describe('override values', () => {
  it('round-trips through the select', () => {
    expect(overrideFrom(overrideValue({ act: 4 }))).toEqual({ act: 4 });
    expect(overrideFrom(overrideValue('have'))).toBe('have');
    expect(overrideFrom(overrideValue(null))).toBeNull();
  });
});

describe('GemRows', () => {
  beforeEach(() => {
    useAppStore.setState({ build: { class_name: 'Witch' } as unknown as BuildSummary });
    vi.mocked(commands.getGemRows).mockResolvedValue([
      { name: 'Fireball', place: 'Quest reward from Tarkleigh in act 1', over: null },
      { name: 'Herald of Ash', place: null, over: 'have' },
    ]);
    vi.mocked(commands.setGemOverride).mockResolvedValue();
  });

  it('counts the gems and opens the list', async () => {
    const user = userEvent.setup();
    render(<GemRows />);
    expect(await screen.findByText('2 in the build, some set by you')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Show' }));
    expect(screen.getByText('Fireball')).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'When to get Fireball' })).toHaveTextContent(
      'Quest reward from Tarkleigh in act 1',
    );
    expect(screen.getByRole('combobox', { name: 'When to get Herald of Ash' })).toHaveTextContent(
      'I have it already',
    );
  });

  it('shows nothing without a build', () => {
    useAppStore.setState({ build: null });
    const { container } = render(<GemRows />);
    expect(container).toBeEmptyDOMElement();
  });
});
