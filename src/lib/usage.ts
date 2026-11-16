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
