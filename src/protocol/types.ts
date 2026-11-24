// The daemon contract as the client sees it. Shapes follow `lumen-core` (camelCase serde).

export type AccessMode = 'read-only' | 'file-edits' | 'autopilot' | 'ask';

export interface Session {
  id: string;
  title: string;
  pinned: boolean;
  /** idle, queued, running, waiting, partial, interrupted, completed, cancelled, failed, error… */
  status: string;
  workspace: string;
  updatedAt: number;
  workspaceId: string | null;
  accessMode: AccessMode;
}

export interface Activity {
  kind: string;
  text: string;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant' | string;
  text: string;
  activity: Activity[];
  /** Highest persisted event already folded into this message. */
  eventSeq: number;
  /** Highest event id in the store when the snapshot was read. */
  snapshotSeq?: number;
  /** Orchestra lanes' live text, by lane id. */
  live?: Record<string, string>;
}

export interface RunEvent {
  sessionId: string;
  /** Empty for session-level events, which are not persisted and carry no `seq`. */
  messageId: string;
  kind: string;
  text: string;
  seq?: number;
  detail?: { digest: string; totalBytes: number; totalLines: number; shownBytes: number };
}

/** How the client is linked to the daemon. Sent by the shell (or the test bridge). */
export type Connection =
  | { state: 'connecting' }
  | { state: 'connected' }
  | { state: 'reconnecting' }
  | { state: 'mismatch'; detail: string }
  | { state: 'failed'; detail: string };

/** One frame of the daemon's event stream, plus the shell's `lumen:connection`. */
export type Frame =
  | { event: 'lumen:run'; payload: RunEvent }
  | { event: 'lumen:connection'; payload: Connection }
  | { event: 'lumen:resync'; payload: unknown }
  | { event: string; payload: unknown };

/** Commands the daemon deduplicates by id; a lost reply leaves their outcome unknown. */
export const receipted = new Set([
  'create_session',
  'send_message',
  'cancel_run',
  'answer_question',
  'gatekeeper_decide',
  'resume_at_reset',
  'orchestra_answer',
  'orchestra_approve',
  'orchestra_cancel',
  'orchestra_resume',
  'orchestra_raise',
]);

/** `lumen_ipc::Client::call` reports a receipted command it could not confirm this way. */
export function unconfirmedId(error: string): string | null {
  return /^Cannot confirm command ([0-9a-f-]{36}):/.exec(error)?.[1] ?? null;
}
