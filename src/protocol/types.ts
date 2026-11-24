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

/** A tool output or diff over the wire limit arrives shortened, with this reference to the rest. */
export interface Detail {
  digest: string;
  totalBytes: number;
  totalLines: number;
  shownBytes: number;
}

export interface Activity {
  kind: string;
  text: string;
  detail?: Detail;
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
  detail?: Detail;
}

export type Provider = 'claude' | 'gpt' | 'antigravity' | 'ollama' | 'opencode' | 'cursor' | 'openaiapi' | 'anthropicapi' | 'googleapi';

export interface Account {
  id: string;
  name: string;
  provider: Provider;
}

export interface ModelInfo {
  id: string;
  name: string;
  description: string;
  efforts: string[];
  defaultEffort: string;
  isDefault: boolean;
}

export type AutoPolicy = 'budget' | 'default' | 'quality';

/** `send_message` options. `mode` is "auto", or Direct with an account. Orchestra is M7. */
export interface RunOptions {
  mode: string;
  accountId?: string | null;
  model?: string | null;
  effort: string;
  autoPolicy?: AutoPolicy;
  accessMode?: AccessMode;
  autoReview?: boolean;
  attachments?: string[];
}

/** The daemon's routing decision, returned by `send_message` when the turn starts. */
export interface Plan {
  provider: Provider;
  accountId: string;
  model: string | null;
  effort: string;
  strategy: string;
  reason: string;
  partnerAccount: string | null;
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
