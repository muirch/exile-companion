import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { Select } from './Select';

const OPTIONS = [
  { value: 'auto', label: 'Auto' },
  { value: 'act1', label: 'From act 1' },
  { value: 'act2', label: 'From act 2' },
  { value: 'endgame', label: 'Endgame' },
];

function setup(value = 'auto'): { onChange: ReturnType<typeof vi.fn> } {
  const onChange = vi.fn();
  render(<Select value={value} options={OPTIONS} label="Stage" onChange={onChange} />);
  return { onChange };
}

describe('Select', () => {
  it('shows the current value and opens a list on click', async () => {
    const { onChange } = setup('act1');
    const box = screen.getByRole('combobox', { name: 'Stage' });
    expect(box).toHaveTextContent('From act 1');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();

    await userEvent.click(box);
    expect(box).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('option', { name: 'From act 1' })).toHaveAttribute('aria-selected', 'true');

    await userEvent.click(screen.getByRole('option', { name: 'Endgame' }));
    expect(onChange).toHaveBeenCalledWith('endgame');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('walks the list with the arrow keys and picks with Enter', async () => {
    const { onChange } = setup();
    screen.getByRole('combobox').focus();
    await userEvent.keyboard('{ArrowDown}');
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    await userEvent.keyboard('{ArrowDown}{ArrowDown}{Enter}');
    expect(onChange).toHaveBeenCalledWith('act2');
  });

  it('jumps to an option by its first letter', async () => {
    const { onChange } = setup();
    screen.getByRole('combobox').focus();
    await userEvent.keyboard('{Enter}e{Enter}');
    expect(onChange).toHaveBeenCalledWith('endgame');
  });

  it('closes on Esc and on a click outside without a change', async () => {
    const { onChange } = setup();
    const box = screen.getByRole('combobox');
    await userEvent.click(box);
    await userEvent.keyboard('{ArrowDown}{Escape}');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(box).toHaveFocus();

    await userEvent.click(box);
    await userEvent.click(document.body);
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('does not report picking the value it already has', async () => {
    const { onChange } = setup('act1');
    await userEvent.click(screen.getByRole('combobox'));
    await userEvent.click(screen.getByRole('option', { name: 'From act 1' }));
    expect(onChange).not.toHaveBeenCalled();
  });
});
