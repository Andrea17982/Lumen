import { isCancelled, unconfirmedId, type Answers, type ApprovalDecision, type ApprovalSnapshot, type Connection, type Frame, type GatekeeperEvent, type Message, type RunOptions, type RunEvent, type Session, type Workspace, type Account, type AccountUsage, type UpdateInfo } from '../protocol/types';
import { errorText, type Transport } from '../transport/types';
import type { OnboardingAction, OnboardingState } from '../protocol/types';

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
  /** False while more sessions match than were read; `showMore` reads the next page. */
  sessionsComplete: boolean;
  /** The search the session list was read with. */
  search: string;
  workspaces: Workspace[];
  accounts: Account[];
  /** Quota as the official CLIs report it, by account. Absent means not reported, never zero. */
  usage: AccountUsage[];
  transcripts: Readonly<Record<string, Transcript>>;
  unconfirmed: Unconfirmed[];
  outgoing: Outgoing[];
  /** Gatekeeper requests and provider connections of each watched session. */
  approvals: Readonly<Record<string, Approvals>>;
  /** Ids of native questions the daemon is still waiting on, across sessions; null until read. */
  questions: readonly string[] | null;
  /** What enabled plugins add: themes, slash commands and panels. */
  contributions: Contributions;
  /** Text a plugin panel asked to put in the composer, until one takes it. */
  insert: { text: string; n: number } | null;
  /** Changes whenever a recurring task is added from this window. */
  routines: number;
  /** A newer signed release, once the shell has found one. */
  update: UpdateInfo | null;
  onboarding?: OnboardingState;
  error?: string;
}

export interface Contributions {
  themes: { plugin: string; id: string; name: string; tokens: Record<string, Record<string, string>> }[];
  slashCommands: { plugin: string; name: string; description: string; prompt: string }[];
  panels: { plugin: string; id: string; title: string; network: string[] }[];
}
const NO_CONTRIBUTIONS: Contributions = { themes: [], slashCommands: [], panels: [] };

export interface Approvals extends ApprovalSnapshot {
  /** The journal could not be read: approvals cannot be shown, so they must not look empty. */
  error?: string;
}

const LIVE_LIMIT = 20_000;
/** Text plugin panels may queue for the composer before one takes it. */
const MAX_INSERT = 20_000;
// Kinds the daemon folds only once when replaying, because the saved message may already hold them.
const SAVED_ONCE = new Set(['interaction', 'interaction-result', 'quota-scheduled', 'browser', 'image']);
const SESSION_REFRESH_MS = 120;
const SESSION_PAGE = 100;
const QUESTION_REFRESH_MS = 60;
// A question appears or closes with one of these; the daemon's list says which are still open.
const QUESTION_KINDS = new Set(['interaction', 'interaction-result', 'start', 'done']);
/** The newest page shows at once; earlier pages follow in the background (M0 strategy). */
export const FIRST_PAGE = 100;
export const PAGE = 1000;
/** Messages read again when a turn ends: the reply and the prompt it answers. */
const TAIL = 2;

export class Sync {
  private state: SyncState = { connection: { state: 'connecting' }, synced: false, stopping: false, sessions: [], sessionsComplete: true, search: '', workspaces: [], accounts: [], usage: [], transcripts: {}, unconfirmed: [], outgoing: [], approvals: {}, questions: null, contributions: NO_CONTRIBUTIONS, insert: null, routines: 0, update: null };
  private listeners = new Set<() => void>();
  /** Events for a transcript whose snapshot is being read; applied after it, deduplicated. */
  private buffers = new Map<string, RunEvent[]>();
  private loads = new Map<string, number>();
  private open = new Set<string>();
  private generation = 0;
  private retries = 0;
  private retryTimer: ReturnType<typeof setTimeout> | undefined;
  private refresh?: ReturnType<typeof setTimeout>;
  private pages = 1;
  private questionRefresh?: ReturnType<typeof setTimeout>;
  /** Approval snapshots being read; true when a change arrived meanwhile and it must be read again. */
  private approvalLoads = new Map<string, boolean>();
  private unsubscribe?: () => void;

  constructor(private transport: Transport) {}

  async start() {
    this.unsubscribe = await this.transport.subscribe((frame) => this.onFrame(frame));
    this.onConnection(await this.transport.connection());
  }

  stop() {
    this.unsubscribe?.();
    clearTimeout(this.refresh);
    clearTimeout(this.questionRefresh);
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
    if (this.state.connection.state === 'connected') this.load(id).catch((error) => this.failed(error));
  }

  unwatch(id: string) {
    this.open.delete(id);
    this.buffers.delete(id);
    this.loads.delete(id);
    const { [id]: _, ...rest } = this.state.transcripts;
    const { [id]: __, ...approvals } = this.state.approvals;
    this.set({ transcripts: rest, approvals });
  }

  /**
   * Sends a command. Failures reject with the daemon's text; an unknown outcome is recorded and
   * blocks new work in `sessionId` (by default the command's `id` argument) until acknowledged.
   */
  async call<T = unknown>(command: string, args: Record<string, unknown> = {}, sessionId = typeof args.id === 'string' ? args.id : undefined): Promise<T> {
    try {
      return (await this.transport.call(command, args)) as T;
    } catch (cause) {
      const error = errorText(cause);
      const id = unconfirmedId(error);
      if (id) {
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
      if (isCancelled(error)) this.dismiss(id);
      else update(unconfirmedId(String(error)) ? { state: 'unconfirmed', error: String(error) } : { state: 'failed', error: String(error) });
      throw error;
    }
  }

  /**
   * Answers a native question. Secret answers pass straight to the daemon: nothing here keeps
   * them, and the daemon leaves them out of the saved result.
   */
  async answer(sessionId: string, id: string, answers: Answers): Promise<void> {
    try {
      await this.call('answer_question', { id, answers }, sessionId);
    } finally {
      this.scheduleQuestions();
    }
  }

  /** Sends one Gatekeeper decision. The broker keeps the request; only its id and scope go back. */
  async decide(decision: ApprovalDecision): Promise<void> {
    await this.call('gatekeeper_decide', { decision }, decision.scope.sessionId);
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
      await Promise.all([this.loadSessions(), this.loadQuestions().catch(() => this.set({ questions: null })), this.loadAccounts(), this.loadContributions(), this.updateOnboarding().catch(() => undefined), ...[...this.open].map((id) => this.load(id))]);
      if (generation === this.generation) {
        this.retries = 0;
        this.set({ synced: true, error: undefined });
      }
    } catch (error) {
      if (generation === this.generation) this.failed(error);
    }
  }

  /**
   * A snapshot read failed: say so, and read everything again a few times with backoff while the
   * service is connected (1, 2, 4, 8 s). After that the banner's Retry is the way forward.
   */
  private failed(error: unknown) {
    this.set({ error: errorText(error) });
    if (this.retryTimer !== undefined || this.retries >= 4 || this.state.connection.state !== 'connected') return;
    const delay = 1000 * 2 ** this.retries++;
    this.retryTimer = setTimeout(() => {
      this.retryTimer = undefined;
      if (this.state.connection.state === 'connected') void this.resync();
    }, delay);
  }

  private onFrame(frame: Frame) {
    switch (frame.event) {
      case 'lumen:connection':
        return this.onConnection(frame.payload as Connection);
      case 'lumen:resync':
        return void this.resync();
      case 'lumen:run':
        return this.onRun(frame.payload as RunEvent);
      case 'lumen:gatekeeper':
        return this.onGatekeeper(frame.payload as GatekeeperEvent);
      case 'lumen:update':
        return this.set({ update: frame.payload as UpdateInfo });
      case 'lumen:onboarding':
        return this.applyOnboarding(frame.payload as OnboardingState);
      case 'lumen:usage': {
        const usage = frame.payload as AccountUsage;
        return this.set({ usage: [...this.state.usage.filter((u) => u.accountId !== usage.accountId), usage] });
      }
    }
  }

  async updateOnboarding(action?: OnboardingAction) {
    this.applyOnboarding(await this.call<OnboardingState>('onboarding_state', action ? { action } : {}));
  }

  private applyOnboarding(onboarding: OnboardingState) {
    if (onboarding.revision > (this.state.onboarding?.revision ?? -1)) this.set({ onboarding });
  }

  /** Accounts and their reported quota; read on every connection and after account changes. */
  async loadAccounts() {
    const [accounts, usage] = await Promise.all([this.transport.call('list_accounts', {}), this.transport.call('get_usage', {})]);
    this.set({ accounts: accounts as Account[], usage: usage as AccountUsage[] });
  }

  /** Read on every connection and after plugin changes. A failure leaves nothing contributed. */
  async loadContributions() {
    const value = (await this.transport.call('plugin_contributions', {}).catch(() => null)) as Partial<Contributions> | null;
    this.set({ contributions: { themes: value?.themes ?? [], slashCommands: value?.slashCommands ?? [], panels: value?.panels ?? [] } });
  }

  /** Bumped when a recurring task is added, so lists of them read again. */
  routinesChanged() {
    this.set({ routines: this.state.routines + 1 });
  }

  /** A plugin panel's `prompt.insert`: it waits here until a composer takes it. */
  insertPrompt(text: string): boolean {
    const waiting = this.state.insert?.text;
    const next = waiting ? `${waiting}
${text}` : text;
    if (next.length > MAX_INSERT) return false;
    this.set({ insert: { text: next, n: (this.state.insert?.n ?? 0) + 1 } });
    return true;
  }

  takeInsert() {
    const text = this.state.insert?.text ?? null;
    if (text !== null) this.set({ insert: null });
    return text;
  }

  private onGatekeeper(event: GatekeeperEvent) {
    if ('resync' in event) return void Promise.all([...this.open].map((id) => this.loadApprovals(id)));
    const sessionId = 'changed' in event ? event.changed.scope.sessionId : event.session.scope.sessionId;
    // The snapshot in flight may be older than this change: read it once more afterwards.
    if (this.approvalLoads.has(sessionId)) return void this.approvalLoads.set(sessionId, true);
    if ('changed' in event) {
      const request = event.changed;
      const current = this.state.approvals[request.scope.sessionId];
      if (!current) return;
      const index = current.requests.findIndex((r) => r.id === request.id);
      const requests = index >= 0 ? current.requests.with(index, request) : [...current.requests, request];
      return this.set({ approvals: { ...this.state.approvals, [request.scope.sessionId]: { ...current, requests } } });
    }
    if ('session' in event) {
      const session = event.session;
      const current = this.state.approvals[session.scope.sessionId];
      if (!current) return;
      const index = current.sessions.findIndex((s) => s.scope.connectionId === session.scope.connectionId);
      const sessions = index >= 0 ? current.sessions.with(index, session) : [...current.sessions, session];
      this.set({ approvals: { ...this.state.approvals, [session.scope.sessionId]: { ...current, sessions } } });
    }
  }

  private onConnection(connection: Connection) {
    const reconnected = connection.state === 'connected' && this.state.connection.state !== 'connected';
    this.set({ connection, ...(connection.state === 'connected' ? { stopping: false } : { synced: false }) });
    if (reconnected) {
      this.retries = 0;
      void this.resync();
    }
  }

  private onRun(event: RunEvent) {
    // Session status is not persisted per event: the list is the truth, so read it again.
    if (!event.messageId || event.kind === 'start' || event.kind === 'status' || event.kind === 'done') this.scheduleSessions();
    if (QUESTION_KINDS.has(event.kind)) this.scheduleQuestions();
    if (!event.messageId || !this.open.has(event.sessionId)) return;
    const buffer = this.buffers.get(event.sessionId);
    if (buffer) return void buffer.push(event);
    // A new turn starts with a user message the stream does not carry: read the snapshot.
    if (event.kind === 'start' && !this.state.transcripts[event.sessionId]?.messages.some((m) => m.id === event.messageId)) {
      return void this.load(event.sessionId).catch((error) => this.failed(error));
    }
    const transcript = this.state.transcripts[event.sessionId];
    if (!transcript) return;
    // The saved reply can hold rows that were never streamed (resume and recovery state).
    if (event.kind === 'done') void this.refreshTail(event.sessionId);
    const next = apply(transcript, event);
    if (next === transcript) return;
    this.set({ transcripts: { ...this.state.transcripts, [event.sessionId]: next } });
    if (next.messages.length !== transcript.messages.length) this.prune(event.sessionId);
  }

  private scheduleSessions() {
    clearTimeout(this.refresh);
    this.refresh = setTimeout(() => {
      this.loadSessions().catch((error) => this.failed(error));
    }, SESSION_REFRESH_MS);
  }

  private scheduleQuestions() {
    clearTimeout(this.questionRefresh);
    this.questionRefresh = setTimeout(() => {
      // Unknown is not "none": the cards say the status could not be read.
      this.loadQuestions().catch(() => this.set({ questions: null }));
    }, QUESTION_REFRESH_MS);
  }

  private async loadQuestions() {
    this.set({ questions: (await this.transport.call('pending_questions', {})) as string[] });
  }

  private async loadApprovals(id: string): Promise<void> {
    if (this.approvalLoads.has(id)) return void this.approvalLoads.set(id, true);
    this.approvalLoads.set(id, false);
    let approvals: Approvals;
    try {
      approvals = (await this.transport.call('gatekeeper_snapshot', { chat: id })) as ApprovalSnapshot;
    } catch (error) {
      approvals = { requests: [], sessions: [], error: errorText(error) };
    }
    const again = this.approvalLoads.get(id);
    this.approvalLoads.delete(id);
    if (!this.open.has(id)) return;
    this.set({ approvals: { ...this.state.approvals, [id]: approvals } });
    if (again) return this.loadApprovals(id);
  }

  private async loadSessions() {
    // M5 adds paging and search; the first page is enough for the shell.
    const search = this.state.search;
    const pages = this.pages;
    const read: Session[] = [];
    let complete = true;
    for (let page = 0; page < pages; page++) {
      const chunk = (await this.transport.call('list_sessions', { search, offset: page * SESSION_PAGE })) as Session[];
      read.push(...chunk);
      complete = chunk.length < SESSION_PAGE;
      if (complete) break;
    }
    // A newer search may have started meanwhile: its own read wins.
    if (search !== this.state.search) return;
    const workspaces = (await this.transport.call('list_workspaces', {})) as Workspace[];
    this.set({ sessions: read, sessionsComplete: complete, workspaces });
  }

  /** The system folder picker, through the shell. */
  pickFolder() {
    return this.transport.pickFolder();
  }

  notify(title: string, body: string) {
    this.transport.notify(title, body);
  }

  panelUrl(plugin: string, panel: string) {
    return this.transport.panelUrl(plugin, panel);
  }

  /** Reads the session list for a search, from its first page. */
  searchSessions(search: string) {
    this.pages = 1;
    this.set({ search });
    return this.loadSessions();
  }

  showMore() {
    this.pages++;
    return this.loadSessions();
  }

  /**
   * Changes a session or project and reads the list again: the daemon sends no event for these.
   * `optimistic` updates the list at once; a refusal puts the daemon's list back.
   */
  async mutate<T>(command: string, args: Record<string, unknown>, optimistic?: (sessions: Session[]) => Session[]): Promise<T> {
    if (optimistic) this.set({ sessions: optimistic(this.state.sessions) });
    try {
      return await this.call<T>(command, args);
    } finally {
      await this.loadSessions().catch((error) => this.failed(error));
    }
  }

  private async load(id: string) {
    // Approvals load beside the transcript; a journal error shows on its own.
    void this.loadApprovals(id);
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

  /** Reads the newest messages again and puts the saved copies in place of the folded ones. */
  async refreshTail(id: string) {
    let tail: Message[];
    try {
      tail = (await this.transport.call('get_messages', { id, limit: TAIL })) as Message[];
    } catch {
      return; // The folded copy stays; the next resync reads everything again.
    }
    const transcript = this.state.transcripts[id];
    if (!transcript || !this.open.has(id) || this.buffers.has(id)) return;
    let messages = transcript.messages;
    for (const saved of tail) {
      const index = messages.findIndex((m) => m.id === saved.id);
      messages = index >= 0 ? messages.with(index, saved) : [...messages, saved];
    }
    this.set({ transcripts: { ...this.state.transcripts, [id]: { ...transcript, messages } } });
    this.prune(id);
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
