import type { OrchestraQueue, WaitReason } from '../../../lib/types';

const LABEL: Record<WaitReason, string> = {
  concurrency: 'Waiting for a free agent slot',
  'total-cap': 'Subtask limit reached',
  probe: 'Waiting while one job checks an account with missing or outdated quota data',
  cooldown: 'Waiting for a rate limit to clear',
  quota: 'Waiting for quota under the configured threshold',
};

/** One line per reason that holds queued work, for the Orchestra card. */
export function describeWaiting(q: OrchestraQueue, now = Date.now()): string[] {
  const groups = new Map<WaitReason, string[]>();
  for (const w of q.waiting) groups.set(w.reason, [...(groups.get(w.reason) ?? []), w.id]);
  const lines: string[] = [];
  for (const [reason, ids] of groups) {
    let text = `${LABEL[reason] ?? 'Waiting'}: ${ids.join(', ')}`;
    if (reason === 'concurrency') text += ` (${q.running} of ${q.concurrent} running)`;
    if (reason === 'total-cap') text += ` (${q.admitted} of ${q.total} admitted)`;
    if ((reason === 'cooldown' || reason === 'quota') && q.wakeAt && q.wakeAt > now) {
      text += ` · checks again at ${new Date(q.wakeAt).toLocaleTimeString()}`;
    }
    lines.push(text);
  }
  return lines;
}

/** Held by the total cap: only an explicit raise admits them. */
export const heldByTotal = (q: OrchestraQueue) =>
  q.waiting.filter((w) => w.reason === 'total-cap').length;
