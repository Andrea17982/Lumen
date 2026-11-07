import type { AccountUsage } from './types';

function hasValidTimestamp(row: AccountUsage, now: number): boolean {
  return (
    Number.isFinite(row.updatedAt) &&
    !Number.isNaN(new Date(row.updatedAt).getTime()) &&
    row.updatedAt <= now
  );
}

export function usageWindowLabel(minutes: number): string {
  if (minutes === 10080) return 'Weekly';
  if (minutes % 1440 === 0) return `${minutes / 1440} day`;
  if (minutes % 60 === 0) return `${minutes / 60} hour`;
  return `${minutes} min`;
}

export function formatResetCountdown(until: number, now = Date.now()): string {
  const minutes = Math.max(0, Math.ceil((until - now) / 60_000));
  return minutes >= 60 ? `${Math.floor(minutes / 60)}h ${minutes % 60}m` : `${minutes}m`;
}

// Match the backend router's 15-minute freshness window.
export function usageStatus(row: AccountUsage | undefined, now = Date.now()): string {
  if (!row) return 'No quota data available';
  const date = new Date(row.updatedAt);
  if (!hasValidTimestamp(row, now)) {
    return 'Quota timestamp unavailable; data may be outdated';
  }
  const label = row.source === 'manual' ? 'Manual snapshot' : 'Last update';
  const stale = now - row.updatedAt > 900_000 ? ' · Outdated' : '';
  return `${label}: ${date.toLocaleString()}${stale}`;
}

export function usageStatusLabel(row: AccountUsage | undefined, now = Date.now()): string {
  if (!row) return 'Unavailable';
  if (!hasValidTimestamp(row, now)) return 'Check data';
  if (now - row.updatedAt > 900_000) return 'Outdated';
  return row.source === 'manual' ? 'Manual' : 'Updated';
}
