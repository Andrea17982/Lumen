import { describe, expect, it } from 'vitest';
import type { OrchestraQueue } from '../../../lib/types';
import { describeWaiting, heldByTotal } from './describe';
import { normalizeLimits, raiseTotal } from './limits';

const now = 1_800_000_000_000;
describe('normalizeLimits', () => {
  it('clamps untrusted values and drops invalid thresholds', () => {
    expect(
      normalizeLimits({
        maxConcurrent: '3',
        maxSubtasks: 0,
        quotaThresholds: { claude: 79.6, gpt: 100, gemini: -1, nope: 50 },
      }),
    ).toEqual({ maxConcurrent: 3, maxSubtasks: null, quotaThresholds: { claude: 80 } });
    expect(normalizeLimits(null)).toEqual({
      maxConcurrent: null,
      maxSubtasks: null,
      quotaThresholds: {},
    });
    expect(normalizeLimits({ maxConcurrent: 1e9 }).maxConcurrent).toBe(32);
  });

  it('raises the total from what is already admitted', () => {
    const l = normalizeLimits({ maxSubtasks: 2 });
    expect(raiseTotal(l, 2, 3).maxSubtasks).toBe(5);
    expect(raiseTotal(l, 4, 1).maxSubtasks).toBe(5);
    expect(raiseTotal(normalizeLimits({}), 4, 1).maxSubtasks).toBeNull();
  });
});

describe('describeWaiting', () => {
  const snap: OrchestraQueue = {
    running: 2,
    admitted: 4,
    concurrent: 2,
    total: 4,
    waiting: [
      { id: 's5', reason: 'total-cap' },
      { id: 's6', reason: 'total-cap' },
      { id: 's8', reason: 'quota' },
    ],
    wakeAt: now + 60_000,
  };
  it('groups held work by reason', () => {
    const lines = describeWaiting(snap, now);
    expect(lines[0]).toBe('Subtask limit reached: s5, s6 (4 of 4 admitted)');
    expect(lines[1]).toMatch(
      /^Waiting for quota under the configured threshold: s8 · checks again/,
    );
    expect(lines).toHaveLength(2);
    expect(heldByTotal(snap)).toBe(2);
  });
});
