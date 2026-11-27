import { describe, expect, it } from 'vitest';
import type { Message } from '../../protocol/types';
import { filesOf, partsOf, roleOf, splitReasoning, stepBody, stepTitle } from './activity';

const message = (text: string, activity: Message['activity']): Message => ({ id: 'm', role: 'assistant', text, activity, eventSeq: 0 });

describe('activity', () => {
  it('names a tool step from the name and its JSON input', () => {
    const step = { kind: 'tool', text: 'Read\n{"file_path":"sample.ts"}' };
    expect(stepTitle(step).detail).toBe('Read sample.ts');
    expect(stepBody(step)).toBe('Read\n{\n  "file_path": "sample.ts"\n}');
  });

  it('finds changed files in both shapes the daemon records', () => {
    expect(filesOf({ kind: 'file', text: 'Write\n{"file_path":"a.rs","content":"x"}' })).toEqual([{ path: 'a.rs', kind: 'Write' }]);
    expect(filesOf({ kind: 'file', text: '[{"path":"sample.ts","kind":"update"}]' })).toEqual([{ path: 'sample.ts', kind: 'update' }]);
  });

  it('shows plain text steps by their first line, and unknown kinds as generic steps', () => {
    expect(stepTitle({ kind: 'command', text: 'npm test\nmore output' }).detail).toBe('npm test');
    expect(roleOf('kind-from-the-future')).toBe('step');
    expect(roleOf('tool-start')).toBe('hidden');
    expect(roleOf('interaction')).toBe('question');
    expect(roleOf('interaction-result')).toBe('hidden');
    expect(roleOf('orchestra')).toBe('later');
  });

  it('pairs each question with its answer, and has no answer for a secret one', () => {
    const request = { id: 'q', questions: [{ id: 'a', header: '', question: 'Which?', options: [], multiSelect: false, secret: false, optional: false }, { id: 'k', header: '', question: 'Key?', options: [], multiSelect: false, secret: true, optional: false }] };
    const parts = partsOf({
      id: 'm',
      role: 'assistant',
      text: '',
      eventSeq: 0,
      activity: [
        { kind: 'interaction', text: JSON.stringify(request) },
        { kind: 'interaction-result', text: JSON.stringify({ id: 'q', answers: { a: ['Native'] } }) },
      ],
    });
    expect(parts.asked).toHaveLength(1);
    expect(parts.asked[0].result?.answers).toEqual({ a: ['Native'] });
    expect(parts.steps).toEqual([]);
  });

  it('separates reasoning, errors, the model line and steps', () => {
    const parts = partsOf(
      message('<think>plan it</think>The answer.<lumen-remember>x</lumen-remember>', [
        { kind: 'model', text: 'claude · Sonnet · low' },
        { kind: 'thinking', text: 'first thought' },
        { kind: 'tool', text: 'Read\n{"file_path":"a"}' },
        { kind: 'usage', text: '1200' },
        { kind: 'error', text: 'Failed once' },
      ]),
    );
    expect(parts.answer).toBe('The answer.');
    expect(parts.reasoning).toBe('first thought\n\nplan it');
    expect(parts.model).toBe('claude · Sonnet · low');
    expect(parts.steps.map((s) => s.kind)).toEqual(['tool']);
    expect(parts.errors).toEqual(['Failed once']);
  });

  it('shows an Autopilot stop as a notice, not as an error or a step', () => {
    const parts = partsOf(message('Needs a credential.', [{ kind: 'autopilot-stopped', text: '{"reason":"blocked"}' }]));
    expect(parts.notices).toEqual(['blocked']);
    expect(parts.errors).toEqual([]);
    expect(parts.steps).toEqual([]);
  });

  it('keeps an unclosed think block as reasoning while it streams', () => {
    expect(splitReasoning('Hi <think>still going')).toEqual({ reasoning: 'still going', answer: 'Hi ' });
  });
});

describe('opening', () => {
  it('names a message by its plain opening text', async () => {
    const { opening } = await import('./activity');
    expect(opening('## Plan\n\nI will **fix** `send_message` now.\n\n```rust\nfn x() {}\n```')).toBe('Plan I will fix send message now.');
    expect(opening('x'.repeat(300)).length).toBe(120);
  });
});
