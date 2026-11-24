import { unconfirmedId, type Connection, type Frame, type Message, type RunOptions, type RunEvent, type Session } from '../protocol/types';
import { errorText, type Transport } from '../transport/types';

/**
 * A cache of daemon state, rebuilt from snapshots whenever the link is (re)established and kept
 * current by the event stream. Nothing here is canonical: a resync can always replace it.
 */
export interface Transcript {
  messages: Message[];
  /** Highest event already reflected; older or repeated events are dropped. */
  seq: number;
  /** False while earlier messages are still being read, newest first, in pages. */
  complete: boolean;
}

export interface Unconfirmed {
  id: string;
  command: string;
  sessionId?: string;
}

/**
 * The one optimistic state: the user's own message, from Send until the daemon's transcript
 * holds it (same id). A failure stays visible with its error; nothing is resent by itself.
 */
export interface Outgoing {
  id: string;
  sessionId: string;
  text: string;
  state: 'sending' | 'sent' | 'failed' | 'unconfirmed';
  error?: string;
}

export interface SyncState {
  connection: Connection;
  /** True once the snapshots read after the latest connection have been applied. */
  synced: boolean;
  /** The daemon refused work because it is shutting down. Cleared by the next connection. */
  stopping: boolean;
  sessions: Session[];
  transcripts: Readonly<Record<string, Transcript>>;
  unconfirmed: Unconfirmed[];
  outgoing: Outgoing[];
  error?: string;
}

const LIVE_LIMIT = 20_000;
// Kinds the daemon folds only once when replaying, because the saved message may already hold them.
const SAVED_ONCE = new Set(['interaction', 'interaction-result', 'quota-scheduled', 'browser', 'image']);
const SESSION_REFRESH_MS = 120;
/** The newest page shows at once; earlier pages follow in the background (M0 strategy). */
export const FIRST_PAGE = 100;
export const PAGE = 1000;

export class Sync {
  private state: SyncState = { connection: { state: 'connecting' }, synced: false, stopping: false, sessions: [], transcripts: {}, unconfirmed: [], outgoing: [] };
  private listeners = new Set<() => void>();
  /** Events for a transcript whose snapshot is being read; applied after it, deduplicated. */
  private buffers = new Map<string, RunEvent[]>();
  private loads = new Map<string, number>();
  private open = new Set<string>();
  private generation = 0;
  private refresh?: ReturnType<typeof setTimeout>;
  private unsubscribe?: () => void;

  constructor(private transport: Transport) {}

  async start() {
    this.unsubscribe = await this.transport.subscribe((frame) => this.onFrame(frame));
    this.onConnection(await this.transport.connection());
  }

  stop() {
    this.unsubscribe?.();
    clearTimeout(this.refresh);
  }

  getState = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  /** Watch a session's transcript. Only watched transcripts are kept and streamed. */
  watch(id: string) {
    if (this.open.has(id)) return;
    this.open.add(id);
    if (this.state.connection.state === 'connected') void this.load(id);
  }

  unwatch(id: string) {
    this.open.delete(id);
    this.buffers.delete(id);
    this.loads.delete(id);
    const { [id]: _, ...rest } = this.state.transcripts;
    this.set({ transcripts: rest });
  }

  /** Sends a command. Failures reject with the daemon's text; an unknown outcome is recorded. */
  async call<T = unknown>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    try {
      return (await this.transport.call(command, args)) as T;
    } catch (cause) {
      const error = errorText(cause);
      const id = unconfirmedId(error);
      if (id) {
        const sessionId = typeof args.id === 'string' ? args.id : undefined;
        this.set({ unconfirmed: [...this.state.unconfirmed, { id, command, sessionId }] });
      } else if (error.includes('different build')) {
        this.set({ connection: { state: 'mismatch', detail: error }, synced: false });
      } else if (error.includes('Lumen is stopping')) {
        this.set({ stopping: true });
      }
      throw error;
    }
  }

  /** True while an earlier command in this session has an unknown outcome. */
  blocked(sessionId: string) {
    return this.state.unconfirmed.some((u) => u.sessionId === sessionId);
  }

  async send(sessionId: string, text: string, options: RunOptions): Promise<void> {
    if (this.blocked(sessionId)) throw 'Check the result of the last action before sending more work';
    const id = crypto.randomUUID();
    const update = (patch: Partial<Outgoing>) => this.set({ outgoing: this.state.outgoing.map((o) => (o.id === id ? { ...o, ...patch } : o)) });
    this.set({ outgoing: [...this.state.outgoing, { id, sessionId, text, state: 'sending' }] });
    try {
      // The reply's own `plan` activity says which agent was chosen and why.
      await this.call('send_message', { id: sessionId, text, options, clientMessageId: id });
      update({ state: 'sent' });
      this.prune(sessionId);
    } catch (error) {
      update(unconfirmedId(String(error)) ? { state: 'unconfirmed', error: String(error) } : { state: 'failed', error: String(error) });
      throw error;
    }
  }

  /** Removes a failed message the user has dismissed. */
  dismiss(id: string) {
    this.set({ outgoing: this.state.outgoing.filter((o) => o.id !== id) });
  }

  /** Outgoing messages the transcript now holds are the daemon's, not ours. */
  private prune(sessionId: string) {
    const messages = this.state.transcripts[sessionId]?.messages;
    if (!messages) return;
    const known = new Set(messages.map((m) => m.id));
    const outgoing = this.state.outgoing.filter((o) => o.sessionId !== sessionId || !known.has(o.id));
    if (outgoing.length !== this.state.outgoing.length) this.set({ outgoing });
  }

  /** The user has checked the result of a command Lumen could not confirm. */
  acknowledge(id: string) {
    this.set({ unconfirmed: this.state.unconfirmed.filter((u) => u.id !== id) });
  }

  /** Re-reads every snapshot. Runs after each connection and on the daemon's `lumen:resync`. */
  async resync() {
    const generation = ++this.generation;
    this.set({ synced: false });
    try {
      await Promise.all([this.loadSessions(), ...[...this.open].map((id) => this.load(id))]);
      if (generation === this.generation) this.set({ synced: true, error: undefined });
    } catch (error) {
      if (generation === this.generation) this.set({ error: errorText(error) });
    }
  }

  private onFrame(frame: Frame) {
    switch (frame.event) {
      case 'lumen:connection':
        return this.onConnection(frame.payload as Connection);
      case 'lumen:resync':
        return void this.resync();
      case 'lumen:run':
        return this.onRun(frame.payload as RunEvent);
    }
    // lumen:gatekeeper (M4) and lumen:usage (M6) have no store yet.
  }

  private onConnection(connection: Connection) {
    const reconnected = connection.state === 'connected' && this.state.connection.state !== 'connected';
    this.set({ connection, ...(connection.state === 'connected' ? { stopping: false } : { synced: false }) });
    if (reconnected) void this.resync();
  }

  private onRun(event: RunEvent) {
    // Session status is not persisted per event: the list is the truth, so read it again.
    if (!event.messageId || event.kind === 'start' || event.kind === 'done') this.scheduleSessions();
    if (!event.messageId || !this.open.has(event.sessionId)) return;
    const buffer = this.buffers.get(event.sessionId);
    if (buffer) return void buffer.push(event);
    // A new turn starts with a user message the stream does not carry: read the snapshot.
    if (event.kind === 'start' && !this.state.transcripts[event.sessionId]?.messages.some((m) => m.id === event.messageId)) {
      return void this.load(event.sessionId);
    }
    const transcript = this.state.transcripts[event.sessionId];
    if (!transcript) return;
    const next = apply(transcript, event);
    if (next === transcript) return;
    this.set({ transcripts: { ...this.state.transcripts, [event.sessionId]: next } });
    if (next.messages.length !== transcript.messages.length) this.prune(event.sessionId);
  }

  private scheduleSessions() {
    clearTimeout(this.refresh);
    this.refresh = setTimeout(() => {
      this.loadSessions().catch((error) => this.set({ error: errorText(error) }));
    }, SESSION_REFRESH_MS);
  }

  private async loadSessions() {
    // M5 adds paging and search; the first page is enough for the shell.
    const sessions = await this.transport.call('list_sessions', { search: '', offset: 0 });
    this.set({ sessions: sessions as Session[] });
  }

  private async load(id: string) {
    const token = (this.loads.get(id) ?? 0) + 1;
    this.loads.set(id, token);
    if (!this.buffers.has(id)) this.buffers.set(id, []);
    let messages: Message[];
    try {
      messages = (await this.transport.call('get_messages', { id, limit: FIRST_PAGE })) as Message[];
    } catch (error) {
      if (this.loads.get(id) === token) this.buffers.delete(id);
      throw error;
    }
    const current = () => this.loads.get(id) === token && this.open.has(id);
    if (!current()) return;
    const buffered = this.buffers.get(id) ?? [];
    this.buffers.delete(id);
    let transcript: Transcript = { messages, seq: highest(messages), complete: messages.length < FIRST_PAGE };
    for (const event of buffered) transcript = apply(transcript, event);
    this.set({ transcripts: { ...this.state.transcripts, [id]: transcript } });
    this.prune(id);
    // Earlier pages: each one was read after anything already applied, so where an id is in both,
    // the page's copy is the newer one and sits in the right place. A page never raises `seq`:
    // its snapshotSeq covers events of newer messages that may still be on their way.
    while (current() && !this.state.transcripts[id].complete) {
      const loaded = this.state.transcripts[id];
      const page = (await this.transport.call('get_messages', { id, before: loaded.messages[0].id, limit: PAGE })) as Message[];
      if (!current()) return;
      const latest = this.state.transcripts[id];
      const ids = new Set(page.map((m) => m.id));
      this.set({
        transcripts: {
          ...this.state.transcripts,
          [id]: { messages: [...page, ...latest.messages.filter((m) => !ids.has(m.id))], seq: latest.seq, complete: page.length < PAGE },
        },
      });
    }
  }

  private set(patch: Partial<SyncState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
}

function highest(messages: Message[]) {
  return messages.reduce((max, m) => Math.max(max, m.snapshotSeq ?? 0, m.eventSeq ?? 0), 0);
}

/** Folds one event into a transcript as the daemon's `replay_event` does. Returns the same object for a dropped event. */
export function apply(transcript: Transcript, event: RunEvent): Transcript {
  if (event.seq !== undefined && event.seq <= transcript.seq) return transcript;
  const seq = Math.max(transcript.seq, event.seq ?? 0);
  const index = transcript.messages.findIndex((m) => m.id === event.messageId);
  const current: Message = index >= 0 ? transcript.messages[index] : { id: event.messageId, role: 'assistant', text: '', activity: [], eventSeq: 0 };
  const message = fold(current, event);
  const messages = index >= 0 ? transcript.messages.with(index, message) : [...transcript.messages, message];
  return { ...transcript, messages, seq };
}

function fold(message: Message, { kind, text, seq, detail }: RunEvent): Message {
  const eventSeq = seq ?? message.eventSeq;
  switch (kind) {
    case 'delta':
      return { ...message, text: message.text + text, eventSeq };
    case 'replace':
      return { ...message, text, eventSeq };
    case 'start':
    case 'done':
    case 'status':
    case 'workspace':
      return eventSeq === message.eventSeq ? message : { ...message, eventSeq };
    case 'orchestra-delta': {
      let delta: { id?: string; text?: string; kind?: string };
      try {
        delta = JSON.parse(text);
      } catch {
        return message;
      }
      if (!delta.id) return message;
      const old = message.live?.[delta.id] ?? '';
      const next = delta.kind === 'replace' ? (delta.text ?? '') : old + (delta.text ?? '');
      return { ...message, live: { ...message.live, [delta.id]: [...next].slice(-LIVE_LIMIT).join('') }, eventSeq };
    }
    default: {
      // Unknown kinds are kept as generic activity rows; M3 renders and merges them.
      let activity = kind === 'orchestra' ? message.activity.filter((a) => a.kind !== 'orchestra') : message.activity;
      if (!(SAVED_ONCE.has(kind) && activity.some((a) => a.kind === kind && a.text === text))) activity = [...activity, detail ? { kind, text, detail } : { kind, text }];
      return { ...message, activity, eventSeq };
    }
  }
}
