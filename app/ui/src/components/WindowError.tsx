import { Component, type ErrorInfo, type ReactNode } from 'react';

import { Button } from '../kit';

/**
 * Catches a window's render error, so data this version doesn't know (a new league's, say) leaves
 * a message instead of a blank window. The other windows keep working.
 */
export class WindowError extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error): { error: Error } {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error('window crashed', error, info.componentStack);
  }

  render(): ReactNode {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div role="alert" className="m-1 flex max-w-[404px] flex-col gap-1.5 bg-black/90 px-3 py-2.5">
        <p className="font-caps text-[13px] text-value">This window ran into something this version doesn&apos;t know.</p>
        <p className="font-body text-[12px] text-plain">The rest of Exile Companion works. An update will fix it.</p>
        <p className="font-body text-[12px] break-words text-corrupted">{error.message}</p>
        <div>
          <Button onClick={() => this.setState({ error: null })}>Try again</Button>
        </div>
      </div>
    );
  }
}
