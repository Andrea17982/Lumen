import { describe, expect, it } from 'vitest';
import { active, mergeUsage, orchestraOf, providerOf } from './messages';
import type { AccountUsage, Message } from './types';

const msg = (activity: Message['activity']): Message => ({
  id: 'm',
  role: 'assistant',
  text: '',
  activity,
});

describe('message helpers', () => {
  it('names the provider from the lane, or the last model after a fallback', () => {
    expect(providerOf(msg([{ kind: 'lane', text: 'codex' }]))).toBe('gpt');
    expect(providerOf(msg([{ kind: 'model', text: 'claude-opus' }]))).toBe('claude');
    expect(
      providerOf(
        msg([
          { kind: 'model', text: 'claude-opus' },
          { kind: 'fallback', text: '' },
          { kind: 'model', text: 'codex-5' },
        ]),
      ),
    ).toBe('gpt');
    expect(providerOf(msg([]))).toBeNull();
  });

  it('reads Orchestra state and ignores broken JSON', () => {
    expect(orchestraOf(msg([{ kind: 'orchestra', text: '{"stage":"plan"}' }]))?.stage).toBe('plan');
    expect(orchestraOf(msg([{ kind: 'orchestra', text: '{' }]))).toBeNull();
  });

  it('keeps the newest usage row per account', () => {
    const row = (accountId: string, updatedAt: number) =>
      ({ accountId, updatedAt, windows: [], source: 'x' }) as AccountUsage;
    const merged = mergeUsage([row('a', 2), row('b', 1)], [row('a', 1), row('b', 3)]);
    expect(merged.map((r) => [r.accountId, r.updatedAt])).toEqual([
      ['a', 2],
      ['b', 3],
    ]);
  });

  it('treats running and waiting as active', () => {
    expect([active('running'), active('waiting'), active('idle'), active()]).toEqual([
      true,
      true,
      false,
      false,
    ]);
  });
});
