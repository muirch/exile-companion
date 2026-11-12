import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';

import { useAppStore } from '../store/useAppStore';
import { CompactPanel, StatusPanel, expLabel } from './StatusPanel';

describe('expLabel', () => {
  it('says nothing at full experience or without a number', () => {
    expect(expLabel(1)).toBeNull();
    expect(expLabel(0.997)).toBeNull();
    expect(expLabel(null)).toBeNull();
  });

  it('rounds to whole percent, with a decimal near the floor', () => {
    expect(expLabel(0.643)).toBe('XP 64%');
    expect(expLabel(0.043)).toBe('XP 4.3%');
  });
});

describe('CompactPanel', () => {
  beforeEach(() => {
    useAppStore.setState({
      campaign: { ...useAppStore.getState().campaign, act: 2, zone: { name: 'The Riverways', id: '1_2_7', act: 2 } },
      exp: 0.5,
    });
  });

  it('adds the experience share to the zone line', () => {
    render(<CompactPanel />);
    expect(screen.getByText('The Riverways · Act 2 · XP 50%')).toBeInTheDocument();
  });
});

describe('log the game changed', () => {
  const campaign = useAppStore.getState().campaign;
  beforeEach(() => useAppStore.setState({ campaign }));

  it('says so in both layouts and stays quiet otherwise', () => {
    const { unmount } = render(<StatusPanel />);
    expect(screen.queryByText(/log changed/)).not.toBeInTheDocument();
    unmount();
    useAppStore.setState({ campaign: { ...campaign, log_unrecognised: true } });
    render(<StatusPanel />);
    expect(screen.getByText(/log changed with the patch/)).toBeInTheDocument();
    render(<CompactPanel />);
    expect(screen.getAllByText(/log changed with the patch/)).toHaveLength(2);
  });
});
