import type { Message } from '../../protocol/types';

// The Orchestra state the daemon publishes on its message as an `orchestra` activity. The daemon
// owns every transition; the client only reads it and sends the next command.
export interface OrchestraQuestion {
  q: string;
  recommended: string;
}
export interface Subtask {
  id: string;
  title: string;
  detail: string;
  files: string[];
  difficulty: string;
  writes: boolean;
  dependsOn: string[];
  accountId: string;
  agent: string;
  model: string;
  effort: string;
  status: string;
  output: string;
}
export interface OrchestraStep {
  name: string;
  status: string;
  detail: string;
}
export interface Waiting {
  id: string;
  reason: string;
}
export interface OrchestraState {
  stage: string;
  task: string;
  questions: OrchestraQuestion[];
  confirm: string;
  summary: string;
  mode: string;
  subtasks: Subtask[];
  planReview: string;
  readOnly: boolean;
  steps: OrchestraStep[];
  queue: { running: number; admitted: number; concurrent: number; total: number | null; waiting: Waiting[]; wakeAt: number | null } | null;
  resumable: boolean;
  notice: string;
  lanes: [string, string, string][];
}

export function orchestraOf(message: Message): OrchestraState | null {
  const activity = message.activity.find((a) => a.kind === 'orchestra');
  if (!activity) return null;
  try {
    const v = JSON.parse(activity.text) as Partial<OrchestraState>;
    if (typeof v.stage !== 'string') return null;
    return {
      stage: v.stage,
      task: v.task ?? '',
      questions: v.questions ?? [],
      confirm: v.confirm ?? '',
      summary: v.summary ?? '',
      mode: v.mode ?? '',
      subtasks: v.subtasks ?? [],
      planReview: v.planReview ?? '',
      readOnly: !!v.readOnly,
      steps: v.steps ?? [],
      queue: v.queue ?? null,
      resumable: !!v.resumable,
      notice: v.notice ?? '',
      lanes: v.lanes ?? [],
    };
  } catch {
    return null;
  }
}

/** Plan edits as the daemon takes them: only what the user changed. Choosing an agent pins it. */
export type PlanEdit = { id: string; remove?: boolean; accountId?: string };
export function planEdits(subtasks: Subtask[], agents: Record<string, string>, removed: Set<string>): PlanEdit[] {
  return subtasks.flatMap((t): PlanEdit[] => {
    if (removed.has(t.id)) return [{ id: t.id, remove: true }];
    const chosen = agents[t.id];
    return chosen && chosen !== t.accountId ? [{ id: t.id, accountId: chosen }] : [];
  });
}

/** Parts held back only by the total limit: raising it by that many lets them start. */
export function overLimit(state: OrchestraState) {
  return state.queue?.waiting.filter((w) => w.reason === 'total-cap').length ?? 0;
}

/** The tail of a lane's live text: enough to follow it, bounded so a long lane stays cheap. */
export function tail(text: string, max = 4000) {
  if (text.length <= max) return text;
  const cut = text.length - max;
  const newline = text.indexOf('\n', cut);
  return '…' + text.slice(newline >= 0 && newline - cut < 200 ? newline : cut);
}
