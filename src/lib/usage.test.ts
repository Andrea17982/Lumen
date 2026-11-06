import { describe, expect, it } from 'vitest';
import { usageStatus } from './usage';
import type { AccountUsage } from './types';

describe('usageStatus', () => {
  const now = 1_800_000_000_000;
  const row: AccountUsage = { accountId: 'a', windows: [], updatedAt: now, source: 'codex-cli' };
  it('distinguishes missing, current and outdated snapshots', () => {
    expect(usageStatus(undefined, now)).toBe('No quota data available');
    expect(usageStatus(row, now)).not.toContain('Outdated');
    expect(usageStatus({ ...row, updatedAt: now - 900_001 }, now)).toContain('Outdated');
    expect(usageStatus({ ...row, source: 'manual' }, now)).toContain('Manual snapshot');
  });
  it('handles invalid and future timestamps without formatting an invalid date', () => {
    expect(usageStatus({ ...row, updatedAt: NaN }, now)).toContain('timestamp unavailable');
    expect(usageStatus({ ...row, updatedAt: now + 1 }, now)).toContain('timestamp unavailable');
  });
});
