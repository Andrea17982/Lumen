import { describe, expect, it } from 'vitest';
import type { Message } from '../../protocol/types';
import { it as it_ } from '../../i18n/it';
import { quotaText } from '../attention/quota';
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

  it('words a quota stop in the user’s language in place of the daemon’s English error', () => {
    const stop = { provider: 'gpt', model: 'gpt-6.2', reason: 'observed', detail: 'You’ve hit your usage limit. Try again at 1:18 PM.' };
    const parts = partsOf(
      message('Partial work.', [
        { kind: 'error', text: 'A tool failed' },
        { kind: 'quota-reset', text: '{"at":1}' },
        { kind: 'quota-alternative', text: '{"accountId":"b"}' },
        { kind: 'quota-stop', text: JSON.stringify(stop) },
        { kind: 'adapter-error', text: '{"schemaVersion":1,"category":"quota","message":"Rate limit reached"}' },
        { kind: 'error', text: 'Rate limit reached: You’ve hit your usage limit. Lumen did not replay.' },
      ]),
    );
    expect(parts.quota).toEqual(stop);
    expect(parts.errors).toEqual(['A tool failed']);
    expect(parts.steps).toEqual([]);
    expect(quotaText(it_, parts.quota!)).toEqual({
      summary: 'Codex ha esaurito la quota. Lumen non ha inviato di nuovo la richiesta perché il lavoro o la risposta erano già iniziati. Il lavoro fatto finora resta.',
      detail: 'Messaggio di Codex: You’ve hit your usage limit. Try again at 1:18 PM.',
    });
  });

  it('keeps the daemon error when the quota note is malformed or the error is not the one after it', () => {
    const failure = { kind: 'adapter-error', text: '{"category":"quota"}' };
    const malformed = partsOf(message('', [{ kind: 'quota-stop', text: '{"provider":"gpt"}' }, failure, { kind: 'error', text: 'Rate limit reached' }]));
    expect(malformed.quota).toBeUndefined();
    expect(malformed.errors).toEqual(['Rate limit reached']);
    const apart = partsOf(
      message('', [{ kind: 'quota-stop', text: '{"provider":"gpt","reason":"observed","detail":""}' }, failure, { kind: 'tool', text: 'x' }, { kind: 'error', text: 'Later failure' }]),
    );
    expect(apart.errors).toEqual(['Later failure']);
  });

  it('keeps every error when the failure was not a quota', () => {
    const parts = partsOf(
      message('', [
        { kind: 'quota-stop', text: '{"provider":"gpt","reason":"observed","detail":""}' },
        { kind: 'adapter-error', text: '{"category":"failed"}' },
        { kind: 'error', text: 'Process exited' },
      ]),
    );
    expect(parts.errors).toEqual(['Process exited']);
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
