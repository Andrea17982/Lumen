import type { AccountUsage, Message, OrchestraState, Status } from './types';

/** A conversation with a turn in progress. */
export const active = (s?: Status) => s === 'running' || s === 'waiting';

/** Activity kinds that feed the UI or the backend but are not listed under Activity. */
export const HIDDEN = [
  'interaction',
  'interaction-result',
  'child',
  'workspace-root',
  'image',
  'thinking',
  'checkpoint',
  'lane',
  'worktree',
  'applied',
  'discarded',
  'attachments',
  'orchestra',
  'orchestra-nogit',
  'usage',
  'references',
  'native',
  'resume',
  'route',
  'validation',
];
export function orchestraOf(m: Message): OrchestraState | null {
  const a = m.activity.find((x) => x.kind === 'orchestra');
  if (!a) return null;
  try {
    return JSON.parse(a.text) as OrchestraState;
  } catch {
    return null;
  }
}
export const laneOf = (m: Message) => m.activity.find((a) => a.kind === 'lane')?.text;
/** Which provider wrote a message, from its lane or the writer's model activity. */
export function providerOf(m: Message): 'claude' | 'gpt' | null {
  const lane = laneOf(m);
  const models = m.activity.filter((a) => a.kind === 'model').map((a) => a.text);
  const fell = m.activity.some((a) => a.kind === 'fallback');
  const src = lane || (fell ? models.at(-1) : models[0]) || '';
  if (src.startsWith('claude')) return 'claude';
  if (src.startsWith('codex')) return 'gpt';
  return null;
}
/** Newest usage row per account. */
export function mergeUsage(current: AccountUsage[], incoming: AccountUsage[]): AccountUsage[] {
  const merged = new Map(current.map((row) => [row.accountId, row]));
  for (const row of incoming) {
    const previous = merged.get(row.accountId);
    if (!previous || row.updatedAt >= previous.updatedAt) merged.set(row.accountId, row);
  }
  return [...merged.values()];
}
