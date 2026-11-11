import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { WindowError } from './WindowError';

let broken = true;

function Fragile(): React.JSX.Element {
  if (broken) throw new Error("Cannot read properties of undefined (reading 'stats')");
  return <p>Tree</p>;
}

describe('WindowError', () => {
  it('shows what broke instead of a blank window, and tries again', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <WindowError>
        <Fragile />
      </WindowError>,
    );
    const alert = screen.getByRole('alert');
    expect(alert).toHaveTextContent("This window ran into something this version doesn't know.");
    expect(alert).toHaveTextContent("reading 'stats'");
    broken = false;
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(screen.getByText('Tree')).toBeInTheDocument();
  });
});
