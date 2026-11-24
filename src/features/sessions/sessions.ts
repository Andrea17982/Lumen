import type { Session, Workspace } from '../../protocol/types';

/** Windows paths compare without case or slash direction. */
const same = (a: string, b: string) => a.replaceAll('\\', '/').replace(/\/$/, '').toLowerCase() === b.replaceAll('\\', '/').replace(/\/$/, '').toLowerCase();

export function inProject(session: Session, project: Workspace): boolean {
  return session.workspaceId ? session.workspaceId === project.id : same(session.workspace, project.folder);
}

/** The remembered project, else the one with the most recent conversation, else the first. */
export function currentProject(workspaces: Workspace[], sessions: Session[], remembered: string | null): Workspace | null {
  const kept = workspaces.find((w) => w.id === remembered);
  if (kept) return kept;
  const latest = [...sessions].sort((a, b) => b.updatedAt - a.updatedAt).find((s) => workspaces.some((w) => inProject(s, w)));
  return (latest && workspaces.find((w) => inProject(latest, w))) || workspaces[0] || null;
}

export type GroupKey = 'pinned' | 'today' | 'yesterday' | 'earlier';
export interface Group {
  key: GroupKey;
  sessions: Session[];
}

/** Pinned first, then by day. Inside a group the daemon's order (which includes manual moves) holds. */
export function groups(sessions: Session[], now: number): Group[] {
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  const today = start.getTime();
  const yesterday = today - 86_400_000;
  const out: Record<GroupKey, Session[]> = { pinned: [], today: [], yesterday: [], earlier: [] };
  for (const s of sessions) {
    const key: GroupKey = s.pinned ? 'pinned' : s.updatedAt >= today ? 'today' : s.updatedAt >= yesterday ? 'yesterday' : 'earlier';
    out[key].push(s);
  }
  return (['pinned', 'today', 'yesterday', 'earlier'] as const).filter((k) => out[k].length).map((key) => ({ key, sessions: out[key] }));
}

/** Moves `id` before `beforeId` (or to the end of its group when null), as the daemon will. */
export function moved(sessions: Session[], id: string, beforeId: string | null): Session[] {
  const moving = sessions.find((s) => s.id === id);
  if (!moving || id === beforeId) return sessions;
  const rest = sessions.filter((s) => s.id !== id);
  const at = beforeId ? rest.findIndex((s) => s.id === beforeId) : rest.length;
  return [...rest.slice(0, at < 0 ? rest.length : at), moving, ...rest.slice(at < 0 ? rest.length : at)];
}

/**
 * The daemon's `reorder_session(id, beforeId)` lists `id` right below `beforeId`, or first when
 * `beforeId` is null. To show `id` right above `target` (or last, when null), name the
 * conversation that will sit right above it.
 */
export function daemonAnchor(sessions: Session[], id: string, target: string | null): string | null {
  // Pinned conversations are ordered apart from the others: anchor within the same kind.
  const pinned = sessions.find((s) => s.id === id)?.pinned;
  const rest = sessions.filter((s) => s.id !== id && s.pinned === pinned);
  if (target === null) return rest.at(-1)?.id ?? null;
  const at = rest.findIndex((s) => s.id === target);
  return at > 0 ? rest[at - 1].id : null;
}

export type Tone = 'running' | 'needsYou' | 'warning' | 'idle';
export function toneOf(status: string): Tone {
  if (status === 'running' || status === 'queued') return 'running';
  if (status === 'waiting') return 'needsYou';
  if (status === 'interrupted' || status === 'partial' || status === 'failed' || status === 'error') return 'warning';
  return 'idle';
}

export const folderName = (folder: string) => folder.replace(/[\\/]+$/, '').split(/[\\/]/).at(-1) || folder;
