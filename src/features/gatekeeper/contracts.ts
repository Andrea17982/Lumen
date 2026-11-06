export type Provider = 'codex' | 'claude';
export interface Scope {
  sessionId: string;
  connectionId: string;
  provider: Provider;
  providerSessionId: string;
  turnId: string;
}
export type Availability =
  | { state: 'available'; version: string; evidence: string }
  | { state: 'unavailable'; reason: string };
export type RequestStatus =
  'pending' | 'approved' | 'rejected' | 'cancelled' | 'timedOut' | 'stale' | 'resolved';
export type Action = 'approve' | 'reject' | 'cancel';
export interface ApprovalRequest {
  id: string;
  scope: Scope;
  providerItemId: string | null;
  kind: 'command' | 'fileChange' | 'tool' | 'unsupported';
  tool: string;
  command: string | null;
  cwd: string;
  reason: string | null;
  diffs: Array<{ path: string; diff: string }>;
  input: unknown;
  canApprove: boolean;
  unavailableReason: string | null;
  whitelistEligible: boolean;
  createdAtMs: number;
  expiresAtMs: number;
  status: RequestStatus;
  delivery: 'none' | 'ready' | 'sent' | 'uncertain';
  decision: Action | null;
  decisionSource: string | null;
  note: string | null;
}
export interface Decision {
  requestId: string;
  scope: Scope;
  action: Action;
  note: string | null;
}
export interface SessionView {
  scope: Scope;
  cwd: string;
  availability: Availability;
}
export interface ExactRule {
  id: string;
  scope: Scope;
  command: string;
  cwd: string;
}
export type GatekeeperEvent = { changed: ApprovalRequest } | { session: SessionView };

export const whitelistCommands = [
  'git status --short',
  'git diff --stat',
  'git diff --check',
  'git rev-parse --show-toplevel',
] as const;

export function sameScope(a: Scope, b: Scope): boolean {
  return (
    a.sessionId === b.sessionId &&
    a.connectionId === b.connectionId &&
    a.provider === b.provider &&
    a.providerSessionId === b.providerSessionId &&
    a.turnId === b.turnId
  );
}

/** Escape invisible formatting and control characters so a command cannot
 * conceal a suffix or reverse the apparent meaning of its text in a card. */
export function visibleText(value: string): string {
  return value.replace(
    /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u200b-\u200f\u202a-\u202e\u2060-\u206f\ufeff]/g,
    (char) => `\\u${char.charCodeAt(0).toString(16).padStart(4, '0')}`,
  );
}

export function decisionBlocker(
  request: ApprovalRequest,
  sessions: SessionView[],
  now = Date.now(),
): string | null {
  if (request.status !== 'pending') return 'This request is already closed.';
  if (request.expiresAtMs <= now) return 'This request has expired.';
  const live = sessions.find((session) => sameScope(session.scope, request.scope));
  if (!live || live.availability.state !== 'available')
    return 'The provider connection is unavailable.';
  return null;
}
