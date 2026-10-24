import { useCallback, useEffect, useState } from 'react';

import { getNextCard } from '../ipc/commands';
import type { NextCard as Card, NextStep } from '../ipc/types';
import { Button } from '../kit';
import { stepDetail } from './chatLog';

const KIND: Record<NextStep['kind'], string> = {
  gem: 'Gem',
  passive: 'Passive',
  gear: 'Gear',
  ascendancy: 'Ascendancy',
};

// The game's own colours: gem teal for gems, unique brown for ascension, white for the rest.
const TINT: Record<NextStep['kind'], string> = {
  gem: 'text-gem',
  passive: 'text-value',
  gear: 'text-value',
  ascendancy: 'text-unique',
};

/** What to change next, measured with Path of Building against the followed guide. No model.
 * Remount it with a new key to measure again. */
export function NextCard(): React.JSX.Element {
  const [card, setCard] = useState<Card | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchCard = useCallback(
    () =>
      getNextCard()
        .then(setCard)
        .finally(() => setLoading(false)),
    [],
  );

  useEffect(() => {
    void fetchCard();
  }, [fetchCard]);

  const load = (): void => {
    setLoading(true);
    void fetchCard();
  };

  return (
    <section aria-label="What's next" className="flex flex-col gap-2">
      <div className="flex min-h-6 items-center gap-2">
        <h2 className="font-caps text-[13px] text-label">What's next</h2>
        {card?.stage && (
          <span className="min-w-0 truncate font-body text-[11px] text-label">{card.stage}</span>
        )}
        <Button className="ml-auto shrink-0" disabled={loading} onClick={load}>
          Refresh
        </Button>
      </div>
      {loading && !card ? (
        <p role="status" className="flex items-center gap-2 font-caps text-[13px] text-plain">
          <span className="chat-pulse" aria-hidden="true" />
          Measuring your build…
        </p>
      ) : (
        <>
          {card?.laterStage && (
            <p className="font-caps text-[13px] text-quest">
              Your level has reached the guide's next stage, {card.laterStage}.
            </p>
          )}
          {card?.missing && (
            <p className="font-body text-[13px] leading-snug text-plain italic">{card.missing}</p>
          )}
          {card && card.steps.length > 0 && (
            <ul className={`flex flex-col gap-1.5 ${loading ? 'opacity-60' : ''}`}>
              {card.steps.map((s) => (
                <li key={`${s.kind}-${s.name}`} className="flex items-baseline gap-3">
                  <span className="w-20 shrink-0 font-caps text-[12px] text-label">{KIND[s.kind]}</span>
                  <span className="flex min-w-0 flex-col">
                    <span className={`font-caps text-[14px] leading-snug ${TINT[s.kind]}`}>{s.name}</span>
                    {stepDetail(s) && (
                      <span className="font-body text-[12px] text-label">{stepDetail(s)}</span>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
}
