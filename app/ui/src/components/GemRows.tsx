import { useEffect, useState } from 'react';

import { getGemRows, setGemOverride } from '../ipc/commands';
import type { GemOverride, GemRow } from '../ipc/types';
import { Button, Note, Row, Select } from '../kit';
import { useAppStore } from '../store/useAppStore';

const ACTS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

export function overrideValue(over: GemOverride | null): string {
  if (over === null) return 'auto';
  if (over === 'have') return 'have';
  return `act${over.act}`;
}

export function overrideFrom(value: string): GemOverride | null {
  if (value === 'have') return 'have';
  const act = /^act(\d+)$/.exec(value);
  return act ? { act: Number(act[1]) } : null;
}

function options(row: GemRow): { value: string; label: string }[] {
  // With an override the plan's place is the override's, so the auto label can't name it.
  const auto = row.over === null ? (row.place ?? 'Not in the campaign') : 'As the plan has it';
  return [
    { value: 'auto', label: auto },
    ...ACTS.map((a) => ({ value: `act${a}`, label: `In time for act ${a}` })),
    { value: 'have', label: 'I have it already' },
  ];
}

// Where each gem of the build comes from, and the player's word when the plan gets it wrong:
// a gem wanted later than its first preset says, or one a mule or a bought copy already covers.
export function GemRows(): React.JSX.Element | null {
  const build = useAppStore((s) => s.build);
  const [rows, setRows] = useState<GemRow[]>([]);
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!build) return;
    void getGemRows().then((r) => setRows(r ?? []));
  }, [build]);
  if (!build || rows.length === 0) return null;

  const change = async (row: GemRow, value: string): Promise<void> => {
    await setGemOverride(row.name, overrideFrom(value));
    setRows((await getGemRows()) ?? []);
  };

  return (
    <>
      <Row label="Gems">
        <span className="min-w-0 flex-1 font-body text-[13px] text-value">
          {rows.length} in the build
          {rows.some((r) => r.over !== null) && ', some set by you'}
        </span>
        <Button aria-expanded={shown} onClick={() => setShown(!shown)}>
          {shown ? 'Hide' : 'Show'}
        </Button>
      </Row>
      {shown && (
        <>
          {rows.map((r) => (
            <Row key={r.name} label="">
              <span className="min-w-0 flex-1 truncate font-body text-[13px] text-value" title={r.name}>
                {r.name}
              </span>
              <Select
                className="w-56"
                label={`When to get ${r.name}`}
                value={overrideValue(r.over)}
                options={options(r)}
                onChange={(v) => void change(r, v)}
              />
            </Row>
          ))}
          <Note>
            Move a gem to the act you want it in, so its reward or purchase doesn't take an earlier
            slot. Pick I have it already for gems from a mule or bought before the run, and the route
            leaves them out.
          </Note>
        </>
      )}
    </>
  );
}
