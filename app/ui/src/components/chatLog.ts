import type { ChatEvent, NextStep, TurnOver } from '../ipc/types';

export interface Exchange {
  question: string;
  answer: string;
  /** The tool running now, or the last one. Cleared when the answer starts. */
  tool: string | null;
  /** The model's latest progress note between tool calls. */
  note: string | null;
  over: TurnOver | null;
}

export const ask = (log: Exchange[], question: string): Exchange[] => [
  ...log,
  { question, answer: '', tool: null, note: null, over: null },
];

function updateLast(log: Exchange[], f: (e: Exchange) => Exchange): Exchange[] {
  const last = log.at(-1);
  if (!last || last.over) return log;
  return [...log.slice(0, -1), f(last)];
}

export function applyEvent(log: Exchange[], event: ChatEvent): Exchange[] {
  return updateLast(log, (e) => {
    switch (event.kind) {
      case 'text':
        return { ...e, answer: e.answer + event.text, tool: null, note: null };
      case 'paragraph':
        return e.answer && !e.answer.endsWith('\n\n') ? { ...e, answer: `${e.answer}\n\n` } : e;
      case 'note':
        return { ...e, note: event.text.trim() };
      case 'tool':
        return { ...e, tool: event.name, note: null };
    }
  });
}

export const finish = (log: Exchange[], over: TurnOver): Exchange[] =>
  updateLast(log, (e) => ({ ...e, over, tool: null, note: null }));

export const isWorking = (log: Exchange[]): boolean => {
  const last = log.at(-1);
  return !!last && !last.over;
};

const TOOL_LABELS: Record<string, string> = {
  player_status: 'Reading where you are',
  what_next: 'Checking what comes next',
  build_overview: 'Reading your build',
  build_details: 'Reading your build',
  defences: 'Checking defences',
  gem_value: 'Measuring gems',
  passive_value: 'Measuring passives',
  tree_audit: 'Looking for weak passives',
  compare_with_guide: 'Comparing with the guide',
  compare_item: 'Comparing the item',
  what_if: 'Trying the change',
  apply_changes: 'Keeping the change',
  build_history: 'Reading the history',
  undo_change: 'Undoing the change',
  export_build: 'Exporting the build',
  load_build: 'Loading a build',
  back_to_my_build: 'Going back to your build',
  use_as_my_build: 'Making the guide your build',
  follow_guide: 'Saving the guide',
  forget_guide: 'Forgetting the guide',
  guide_stages: 'Reading the guide',
  guide_stage_diff: 'Reading the guide',
  find_guides: 'Looking for guides',
  wiki_search: 'Searching the wiki',
  wiki_page: 'Reading the wiki',
  wiki_data: 'Reading the wiki',
};

export const toolLabel = (name: string | null): string =>
  (name && TOOL_LABELS[name]) || 'Thinking';

export type Block = { kind: 'text'; text: string } | { kind: 'list'; items: string[] };

/** The answer's first paragraph is the point. The rest is details the player can open. */
export interface Answer {
  lead: Block[];
  details: Block[];
}

const BULLET = /^\s*(?:[-*•]|\d+[.)])\s+/;

// The prompt asks for plain text, but a stray **bold** or # heading still slips through.
const clean = (line: string): string => line.replace(/\*\*(.+?)\*\*/g, '$1').replace(/^#+\s+/, '');

function blocks(paragraph: string): Block[] {
  const out: Block[] = [];
  for (const raw of paragraph.split('\n')) {
    const line = clean(raw.trimEnd());
    if (!line.trim()) continue;
    if (BULLET.test(line)) {
      const item = line.replace(BULLET, '');
      const last = out.at(-1);
      if (last?.kind === 'list') last.items.push(item);
      else out.push({ kind: 'list', items: [item] });
    } else {
      const last = out.at(-1);
      if (last?.kind === 'text') last.text += `\n${line}`;
      else out.push({ kind: 'text', text: line });
    }
  }
  return out;
}

export function splitAnswer(answer: string): Answer {
  const paragraphs = answer
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .filter(Boolean);
  return {
    lead: paragraphs.length ? blocks(paragraphs[0]) : [],
    details: paragraphs.slice(1).flatMap(blocks),
  };
}

/** Under a cent reads as "<1¢", so a cheap answer doesn't look free. */
export function costLabel(dollars: number | null): string | null {
  if (dollars === null) return null;
  const cents = dollars * 100;
  if (cents < 1) return '<1¢';
  if (cents < 100) return `${Math.round(cents)}¢`;
  return `$${dollars.toFixed(2)}`;
}

export function overText(over: TurnOver): string | null {
  switch (over.stop) {
    case 'done':
      return null;
    case 'cut':
      return 'The answer was cut off. Ask for less at once.';
    case 'refused':
      return 'The model declined this question.';
    case 'tooManySteps':
      return 'Stopped after too many tool calls. Ask a narrower question.';
    case 'empty':
      return 'The model gave no answer. Ask again, or pick a stronger model.';
    case 'stopped':
      return 'Stopped.';
    case 'error':
      return over.message ?? 'Something went wrong. Ask again.';
  }
}

const pct = (v: number): string => `+${Math.abs(v) >= 10 ? Math.round(v) : Number(v.toFixed(1))}%`;

/** "8 points · +27% EHP · +2.8% DPS": what a step costs and gives, gains under 0.5% left out. */
export function stepDetail(step: NextStep): string {
  const parts = [
    step.slot,
    step.points ? `${step.points} ${step.points === 1 ? 'point' : 'points'}` : null,
    step.ehpPct >= 0.5 ? `${pct(step.ehpPct)} EHP` : null,
    step.dpsPct >= 0.5 ? `${pct(step.dpsPct)} DPS` : null,
  ];
  return parts.filter(Boolean).join(' · ');
}
