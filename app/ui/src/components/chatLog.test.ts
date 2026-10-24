import { describe, expect, it } from 'vitest';

import type { TurnOver } from '../ipc/types';
import { applyEvent, ask, costLabel, finish, isWorking, splitAnswer, toolLabel } from './chatLog';

const done: TurnOver = { stop: 'done', message: null, usage: null, dollars: 0.04, model: 'Sonnet 5.5' };

describe('chat log', () => {
  it('shows the running tool until the answer starts', () => {
    let log = ask([], 'What next?');
    log = applyEvent(log, { kind: 'tool', name: 'passive_value' });
    log = applyEvent(log, { kind: 'note', text: 'Comparing with the guide. ' });
    expect(log[0].tool).toBe('passive_value');
    expect(log[0].note).toBe('Comparing with the guide.');
    log = applyEvent(log, { kind: 'text', text: 'Take ' });
    log = applyEvent(log, { kind: 'text', text: 'Herbalism.' });
    expect(log[0]).toMatchObject({ answer: 'Take Herbalism.', tool: null, note: null });
    expect(isWorking(log)).toBe(true);
    log = finish(log, done);
    expect(isWorking(log)).toBe(false);
  });

  it('puts a blank line between text blocks, once', () => {
    let log = ask([], 'q');
    log = applyEvent(log, { kind: 'paragraph' });
    log = applyEvent(log, { kind: 'text', text: 'One.' });
    log = applyEvent(log, { kind: 'paragraph' });
    log = applyEvent(log, { kind: 'paragraph' });
    log = applyEvent(log, { kind: 'text', text: 'Two.' });
    expect(log[0].answer).toBe('One.\n\nTwo.');
  });

  it('leaves a finished exchange alone', () => {
    const log = finish(applyEvent(ask([], 'q'), { kind: 'text', text: 'A.' }), done);
    expect(applyEvent(log, { kind: 'text', text: 'late' })).toBe(log);
  });

  it('names tools the way a player would', () => {
    expect(toolLabel('compare_item')).toBe('Comparing the item');
    expect(toolLabel('what_next')).toBe('Checking what comes next');
    expect(toolLabel('something_new')).toBe('Thinking');
    expect(toolLabel(null)).toBe('Thinking');
  });

  it('splits the point from the details', () => {
    const a = splitAnswer(
      'Take **Herbalism** next.\n\nIt adds 12% life.\n- 2 points away\n- the guide takes it at 40\n\n## Later\nArcane Will.',
    );
    expect(a.lead).toEqual([{ kind: 'text', text: 'Take Herbalism next.' }]);
    expect(a.details).toEqual([
      { kind: 'text', text: 'It adds 12% life.' },
      { kind: 'list', items: ['2 points away', 'the guide takes it at 40'] },
      { kind: 'text', text: 'Later\nArcane Will.' },
    ]);
  });

  it('prices answers in cents', () => {
    expect(costLabel(0.004)).toBe('<1¢');
    expect(costLabel(0.126)).toBe('13¢');
    expect(costLabel(1.5)).toBe('$1.50');
    expect(costLabel(null)).toBeNull();
  });
});
