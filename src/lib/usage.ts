import type { AccountUsage } from './types';

// Match the backend router's 15-minute freshness window.
export function usageStatus(row: AccountUsage | undefined, now = Date.now()): string {
  if (!row) return 'No quota data available';
  const date = new Date(row.updatedAt);
  if (!Number.isFinite(row.updatedAt) || Number.isNaN(date.getTime()) || row.updatedAt > now) {
    return 'Quota timestamp unavailable; data may be outdated';
  }
  const label = row.source === 'manual' ? 'Manual snapshot' : 'Last update';
  const stale = now - row.updatedAt > 900_000 ? ' · Outdated' : '';
  return `${label}: ${date.toLocaleString()}${stale}`;
}
