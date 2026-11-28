import { isSessionActive, type ApprovalRequest, type ApprovalSession, type Interaction, type InteractionResult, type Message, type Scope } from '../../protocol/types';
import { alternativeOf, type QuotaAlternative } from './quota';

/**
 * Invisible formatting and control characters shown as escapes, so a command cannot hide a
 * suffix or reverse how its text reads on the card the user approves.
 */
export function visibleText(value: string): string {
  return value.replace(HIDDEN, (c) => {
    const code = c.codePointAt(0)!;
    return code > 0xffff ? `\\u{${code.toString(16)}}` : `\\u${code.toString(16).padStart(4, '0')}`;
  });
}
// Controls and format characters (bidi marks, zero-width, tags), line and paragraph separators,
// spaces other than the plain one, variation selectors, and characters that draw as blank.
const HIDDEN = /(?![\t\n])[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]|(?! )\p{Zs}|[\u034f\u115f\u1160\u180e\u2800\u3164\uffa0\ufe00-\ufe0f]|[\u{e0100}-\u{e01ef}]/gu;

export const agentName = (scope: Scope) => (scope.provider === 'claude' ? 'Claude' : 'Codex');

const sameScope = (a: Scope, b: Scope) =>
  a.sessionId === b.sessionId && a.connectionId === b.connectionId && a.provider === b.provider && a.providerSessionId === b.providerSessionId && a.turnId === b.turnId;

export type Blocker = 'expired' | 'disconnected' | 'workerGone';

/** Why a pending request cannot be decided now. Mounting or counting down never decides anything. */
export function blocker(request: ApprovalRequest, sessions: ApprovalSession[], now: number): Blocker | null {
  if (request.status !== 'pending') return null;
  if (request.expiresAtMs <= now) return 'expired';
  const live = sessions.find((s) => sameScope(s.scope, request.scope));
  if (!live || live.availability.state !== 'available') return request.worker ? 'workerGone' : 'disconnected';
  return null;
}

/**
 * Requests to show: open ones, ones whose delivery is uncertain, and ones the user saw open in
 * this view (so a decision or an expiry reads as an outcome rather than a card that vanishes).
 */
export function shown(requests: ApprovalRequest[], seen: ReadonlySet<string>, sessionId: string): ApprovalRequest[] {
  return requests.filter((r) => r.scope.sessionId === sessionId && (r.status === 'pending' || r.delivery === 'uncertain' || seen.has(r.id)));
}

export function countdown(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
}

export type QuestionState = 'open' | 'unknown';

export interface OpenQuestion {
  request: Interaction;
  messageId: string;
  state: QuestionState;
}

// Questions belong to the turn that asked them: only the newest messages can still be waiting.
const RECENT = 4;

/**
 * Questions the agent may still be waiting on: asked in a recent message, with no result, and in
 * the daemon's list of open questions. When that list could not be read, they show as unknown.
 */
export function openQuestions(messages: readonly Message[], pending: readonly string[] | null): OpenQuestion[] {
  const out: OpenQuestion[] = [];
  for (const message of messages.slice(-RECENT)) {
    const answered = new Set(message.activity.filter((a) => a.kind === 'interaction-result').map((a) => parse<InteractionResult>(a.text)?.id));
    for (const a of message.activity) {
      if (a.kind !== 'interaction') continue;
      const request = parse<Interaction>(a.text);
      if (!request?.id || !Array.isArray(request.questions) || answered.has(request.id)) continue;
      if (pending === null) out.push({ request, messageId: message.id, state: 'unknown' });
      else if (pending.includes(request.id)) out.push({ request, messageId: message.id, state: 'open' });
    }
  }
  return out;
}

/** Answers ready to send: chosen options, plus the typed answer when there is one. */
export function collect(request: Interaction, chosen: Record<string, string[]>, typed: Record<string, string>): Record<string, string[]> {
  return Object.fromEntries(
    request.questions.map((q) => {
      const text = typed[q.id]?.trim();
      return [q.id, text ? [...(q.multiSelect ? (chosen[q.id] ?? []) : []), text] : (chosen[q.id] ?? [])];
    }),
  );
}

/** True when every question that needs an answer has one. A declined connector needs only its choice. */
export function complete(request: Interaction, answers: Record<string, string[]>): boolean {
  const declined = request.kind === 'connector' && ['Decline', 'Cancel'].includes(answers[request.actionId ?? '']?.[0] ?? '');
  return request.questions.every((q) => q.optional || (declined && q.id !== request.actionId) || (answers[q.id]?.length ?? 0) > 0);
}

function parse<T>(text: string): T | null {
  try {
    return JSON.parse(text) as T;
  } catch {
    return null;
  }
}

export type Recovery =
  | {
      kind: 'quota';
      messageId: string;
      /** Resume the same native session once at the reset. */
      resume: { at: number; estimated: boolean } | null;
      /** Continue the task in a new turn on the equivalent model Lumen proposed elsewhere. */
      elsewhere: QuotaAlternative | null;
    }
  | { kind: 'scheduled'; at: number }
  | { kind: 'interrupted' };

/**
 * What the latest reply offers after it stopped. A quota stop with a native session can resume
 * once at the reset, and one with a proposed equivalent model can continue there, each on
 * request; any other interruption is shown and never continued by itself.
 */
export function recovery(messages: readonly Message[], status: string): Recovery | null {
  const last = messages.at(-1);
  if (!last || last.role !== 'assistant') return null;
  const latest = <T = { at?: unknown; state?: unknown; estimated?: unknown }>(kind: string) => {
    const row = last.activity.findLast((a) => a.kind === kind);
    return row ? parse<T>(row.text) : null;
  };
  const scheduled = latest('quota-scheduled');
  if (status === 'waiting' && scheduled?.state === 'waiting' && typeof scheduled.at === 'number') return { kind: 'scheduled', at: scheduled.at };
  if (isSessionActive(status)) return null;
  // Only a reply that ended in an error or an interruption can still be recovered.
  const stoppedOnQuota = (status === 'error' || status === 'interrupted') && last.activity.some((a) => a.kind === 'error') && scheduled?.state !== 'resuming';
  const reset = latest('quota-reset');
  const native = last.activity.some((a) => a.kind === 'native');
  const resume = stoppedOnQuota && native && reset && typeof reset.at === 'number' && reset.at > 0 ? { at: reset.at, estimated: reset.estimated === true } : null;
  const elsewhere = stoppedOnQuota ? alternativeOf(latest<unknown>('quota-alternative')) : null;
  if (resume || elsewhere) return { kind: 'quota', messageId: last.id, resume, elsewhere };
  return status === 'interrupted' ? { kind: 'interrupted' } : null;
}
