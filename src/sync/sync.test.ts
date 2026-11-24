import { describe, expect, it, vi } from 'vitest';
import type { ApprovalRequest, Connection, Frame, Message, RunEvent } from '../protocol/types';
import type { Transport } from '../transport/types';
import { apply, FIRST_PAGE, PAGE, Sync } from './sync';

const defaults: Record<string, () => unknown> = { list_workspaces: () => [], list_accounts: () => [], get_usage: () => [], pending_questions: () => [], gatekeeper_snapshot: () => ({ requests: [], sessions: [] }) };

/** A daemon stand-in: replies come from `handlers`, frames are pushed with `emit`. */
function fakeDaemon(handlers: Record<string, (args: Record<string, unknown>) => unknown>, connection: Connection = { state: 'connected' }) {
  let listener: (frame: Frame) => void = () => {};
  const calls: string[] = [];
  const transport: Transport = {
    async call(command, args = {}) {
      calls.push(command);
      // Commands a test does not care about answer as an empty daemon would.
      const handler = handlers[command] ?? defaults[command];
      if (!handler) throw `Unknown command ${command}`;
      return handler(args);
    },
    async subscribe(next) {
      listener = next;
      return () => {};
    },
    connection: async () => connection,
    pickFolder: async () => null,
  };
  return { transport, calls, emit: (frame: Frame) => listener(frame) };
}

const ev = (seq: number | undefined, kind: string, text: string, messageId = 'm2'): RunEvent => ({ sessionId: 's1', messageId, kind, text, seq });
const run = (...args: Parameters<typeof ev>): Frame => ({ event: 'lumen:run', payload: ev(...args) });
const user: Message = { id: 'm1', role: 'user', text: 'Hello', activity: [], eventSeq: 0, snapshotSeq: 10 };
const partial = (text: string, seq: number): Message => ({ id: 'm2', role: 'assistant', text, activity: [], eventSeq: seq, snapshotSeq: seq });
const session = (status: string) => ({ id: 's1', title: 'Fixture', pinned: false, status, workspace: 'C:/w', updatedAt: 1, workspaceId: null, accessMode: 'read-only' });
const settle = () => new Promise((r) => setTimeout(r, 0));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

describe('apply', () => {
  const base = { messages: [user], seq: 10, complete: true };

  it('drops repeated and older events, and keeps the transcript object', () => {
    expect(apply(base, ev(10, 'delta', 'x'))).toBe(base);
    expect(apply(base, ev(3, 'delta', 'x'))).toBe(base);
  });

  it('appends, replaces and keeps finished messages stable by reference', () => {
    const a = apply(base, ev(11, 'delta', 'Hi'));
    const b = apply(a, ev(12, 'delta', ' there'));
    expect(b.messages[1].text).toBe('Hi there');
    expect(b.messages[0]).toBe(user);
    expect(apply(b, ev(13, 'replace', 'Final')).messages[1].text).toBe('Final');
  });

  it('keeps unknown kinds as activity, never failing', () => {
    const next = apply(base, ev(11, 'kind-from-the-future', '{"a":1}'));
    expect(next.messages[1].activity).toEqual([{ kind: 'kind-from-the-future', text: '{"a":1}' }]);
  });

  it('keeps the last 20,000 characters of an Orchestra lane', () => {
    const long = 'x'.repeat(19_990);
    let t = apply(base, ev(11, 'orchestra-delta', JSON.stringify({ id: 'lane', text: long })));
    t = apply(t, ev(12, 'orchestra-delta', JSON.stringify({ id: 'lane', text: 'abcdefghijklmnop' })));
    expect(t.messages[1].live!.lane.length).toBe(20_000);
    expect(t.messages[1].live!.lane.endsWith('abcdefghijklmnop')).toBe(true);
  });
});

describe('Sync', () => {
  it('buffers events while the snapshot is read and applies only the newer ones', async () => {
    const snapshot = deferred<Message[]>();
    const daemon = fakeDaemon({ list_sessions: () => [session('running')], get_messages: () => snapshot.promise });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    // Both arrive before the snapshot; 11 is already inside it, 12 is not.
    daemon.emit(run(11, 'delta', 'Hel'));
    daemon.emit(run(12, 'delta', 'lo'));
    snapshot.resolve([user, partial('Hel', 11)]);
    await settle();
    expect(sync.getState().transcripts.s1.messages[1].text).toBe('Hello');
    expect(sync.getState().synced).toBe(true);
    daemon.emit(run(12, 'delta', 'lo'));
    expect(sync.getState().transcripts.s1.messages[1].text).toBe('Hello');
  });

  it('after a reconnection mid-stream shows the persisted partial text once, with the new status', async () => {
    let status = 'running';
    let stored = [user, partial('Partial', 12)];
    const daemon = fakeDaemon({ list_sessions: () => [session(status)], get_messages: () => stored });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    await settle();
    daemon.emit(run(13, 'delta', ' answer'));
    expect(sync.getState().transcripts.s1.messages[1].text).toBe('Partial answer');

    // The daemon dies; its restart marks the turn interrupted and keeps what it had persisted.
    daemon.emit({ event: 'lumen:connection', payload: { state: 'reconnecting' } });
    expect(sync.getState().synced).toBe(false);
    status = 'interrupted';
    stored = [user, partial('Partial answer', 13)];
    daemon.emit({ event: 'lumen:connection', payload: { state: 'connected' } });
    daemon.emit(run(13, 'delta', ' answer'));
    await settle();
    const state = sync.getState();
    expect(state.synced).toBe(true);
    expect(state.sessions[0].status).toBe('interrupted');
    expect(state.transcripts.s1.messages.map((m) => m.text)).toEqual(['Hello', 'Partial answer']);
  });

  it('reads every snapshot again on the daemon resync event', async () => {
    const daemon = fakeDaemon({ list_sessions: () => [session('idle')], get_messages: () => [user] });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    await settle();
    daemon.calls.length = 0;
    daemon.emit({ event: 'lumen:resync', payload: {} });
    await settle();
    expect(daemon.calls.sort()).toEqual(['gatekeeper_snapshot', 'get_messages', 'get_usage', 'list_accounts', 'list_sessions', 'list_workspaces', 'pending_questions']);
  });

  it('reads the session list again when a turn starts or ends', async () => {
    vi.useFakeTimers();
    try {
      let status = 'running';
      const daemon = fakeDaemon({ list_sessions: () => [session(status)], get_messages: () => [user] });
      const sync = new Sync(daemon.transport);
      await sync.start();
      await vi.runAllTimersAsync();
      status = 'completed';
      daemon.emit(run(14, 'done', 'completed', 'other-message'));
      await vi.runAllTimersAsync();
      expect(sync.getState().sessions[0].status).toBe('completed');
    } finally {
      vi.useRealTimers();
    }
  });

  it('records a command Lumen could not confirm until the user has checked it', async () => {
    const id = '3f2b8c1e-8d4a-4f7e-9a51-1c2d3e4f5a6b';
    const daemon = fakeDaemon({
      list_sessions: () => [],
      send_message: () => {
        throw `Cannot confirm command ${id}: IPC closed. Review its result before submitting new work.`;
      },
    });
    const sync = new Sync(daemon.transport);
    await sync.start();
    await expect(sync.call('send_message', { id: 's1', text: 'Go' })).rejects.toMatch(/Cannot confirm/);
    expect(sync.getState().unconfirmed).toEqual([{ id, command: 'send_message', sessionId: 's1' }]);
    sync.acknowledge(id);
    expect(sync.getState().unconfirmed).toEqual([]);
  });

  it('reports another service build and a stopping service', async () => {
    const daemon = fakeDaemon({
      list_sessions: () => [],
      get_settings: () => {
        throw 'The running Lumen service uses a different build; stop it before updating';
      },
      list_accounts: () => {
        throw 'Lumen is stopping; reconnect after it restarts';
      },
    });
    const sync = new Sync(daemon.transport);
    await sync.start();
    await sync.call('list_accounts').catch(() => {});
    expect(sync.getState().stopping).toBe(true);
    await sync.call('get_settings').catch(() => {});
    expect(sync.getState().connection.state).toBe('mismatch');
    daemon.emit({ event: 'lumen:connection', payload: { state: 'reconnecting' } });
    daemon.emit({ event: 'lumen:connection', payload: { state: 'connected' } });
    expect(sync.getState().stopping).toBe(false);
  });

  it('ignores a stale snapshot that finishes after a newer one', async () => {
    const first = deferred<Message[]>();
    const second = deferred<Message[]>();
    const replies = [first.promise, second.promise];
    const daemon = fakeDaemon({ list_sessions: () => [], get_messages: () => replies.shift() });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    daemon.emit({ event: 'lumen:resync', payload: {} });
    second.resolve([user, partial('New', 20)]);
    await settle();
    first.resolve([user, partial('Old', 5)]);
    await settle();
    expect(sync.getState().transcripts.s1.messages[1].text).toBe('New');
  });

  it('reads a long session newest first, then earlier pages, without losing live events', async () => {
    const stored: Message[] = Array.from({ length: FIRST_PAGE + PAGE + 7 }, (_, i) => ({ id: `m${i}`, role: i % 2 ? 'assistant' : 'user', text: `#${i}`, activity: [], eventSeq: 0, snapshotSeq: 50 }));
    const latest = stored.at(-1)!.id;
    const pages: unknown[] = [];
    let daemon: ReturnType<typeof fakeDaemon>;
    daemon = fakeDaemon({
      list_sessions: () => [session('running')],
      get_messages: ({ before, limit }) => {
        pages.push({ before, limit });
        const end = before ? stored.findIndex((m) => m.id === before) : stored.length;
        const page = stored.slice(Math.max(0, end - (limit as number)), end);
        // While an earlier page is read, a delta for the newest message is still in flight,
        // with a seq below that page's snapshotSeq.
        if (before) {
          daemon.emit({ event: 'lumen:run', payload: { sessionId: 's1', messageId: latest, kind: 'delta', text: '+', seq: 40 + pages.length } });
          return page.map((m) => ({ ...m, snapshotSeq: 60 }));
        }
        return page.map((m) => ({ ...m, snapshotSeq: 30 }));
      },
    });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    await settle();
    await settle();
    const transcript = sync.getState().transcripts.s1;
    expect(transcript.complete).toBe(true);
    expect(transcript.messages.map((m) => m.id)).toEqual(stored.map((m) => m.id));
    expect(pages).toEqual([{ before: undefined, limit: FIRST_PAGE }, { before: stored[7 + PAGE].id, limit: PAGE }, { before: 'm7', limit: PAGE }]);
    expect(transcript.messages.at(-1)!.text).toBe(`#${stored.length - 1}++`);
  });

  it('keeps approvals current from the snapshot and Gatekeeper events, reading again after a change mid-read', async () => {
    const request = (id: string, status = 'pending', sessionId = 's1') => ({ id, scope: { sessionId, connectionId: 'c1', provider: 'claude', providerSessionId: 'p', turnId: 't' }, status }) as unknown as ApprovalRequest;
    const reads: ReturnType<typeof deferred<unknown>>[] = [];
    const daemon = fakeDaemon({
      list_sessions: () => [session('running')],
      get_messages: () => [user],
      pending_questions: () => [],
      gatekeeper_snapshot: () => {
        const read = deferred<unknown>();
        reads.push(read);
        return read.promise;
      },
    });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    await settle();
    expect(reads).toHaveLength(1);
    // A change while the snapshot is in flight: the snapshot may predate it.
    daemon.emit({ event: 'lumen:gatekeeper', payload: { changed: request('a') } });
    reads[0].resolve({ requests: [], sessions: [] });
    await settle();
    expect(reads).toHaveLength(2);
    reads[1].resolve({ requests: [request('a')], sessions: [] });
    await settle();
    expect(sync.getState().approvals.s1.requests.map((r) => r.id)).toEqual(['a']);
    daemon.emit({ event: 'lumen:gatekeeper', payload: { changed: request('a', 'approved') } });
    daemon.emit({ event: 'lumen:gatekeeper', payload: { changed: request('b', 'pending', 'other') } });
    expect(sync.getState().approvals.s1.requests.map((r) => [r.id, r.status])).toEqual([['a', 'approved']]);
    expect(sync.getState().approvals.other).toBeUndefined();
  });

  it('shows an unreadable approval journal as an error, not as no approvals', async () => {
    const daemon = fakeDaemon({ list_sessions: () => [], get_messages: () => [], pending_questions: () => [], gatekeeper_snapshot: () => Promise.reject('unavailable: The approval journal could not be opened.') });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    await settle();
    expect(sync.getState().approvals.s1.error).toMatch(/journal/);
  });

  it('reads the open questions again when one is asked or answered, and blocks the session on an unknown answer outcome', async () => {
    vi.useFakeTimers();
    let open = ['q1'];
    const daemon = fakeDaemon({
      list_sessions: () => [session('waiting')],
      get_messages: () => [user],
      gatekeeper_snapshot: () => ({ requests: [], sessions: [] }),
      pending_questions: () => open,
      answer_question: () => Promise.reject('Cannot confirm command 0b7c4a7e-1f43-4d2b-9a40-3f6a1c2d9e10: transport closed. Review its result before submitting new work.'),
    });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    await vi.runAllTimersAsync();
    expect(sync.getState().questions).toEqual(['q1']);
    open = [];
    await expect(sync.answer('s1', 'q1', { a: ['secret value'] })).rejects.toMatch(/Cannot confirm/);
    await vi.runAllTimersAsync();
    expect(sync.getState().questions).toEqual([]);
    expect(sync.blocked('s1')).toBe(true);
    // Nothing the client keeps holds the answer.
    expect(JSON.stringify(sync.getState())).not.toContain('secret value');
    vi.useRealTimers();
  });

  it('puts the saved reply in place of the folded one when a turn ends', async () => {
    let saved: Message[] = [user, partial('Hi', 11)];
    const daemon = fakeDaemon({ list_sessions: () => [session('running')], get_messages: () => saved, gatekeeper_snapshot: () => ({ requests: [], sessions: [] }), pending_questions: () => [] });
    const sync = new Sync(daemon.transport);
    sync.watch('s1');
    await sync.start();
    await settle();
    // Saved but never streamed: the native session a quota resume needs.
    saved = [user, { ...partial('Hi', 12), activity: [{ kind: 'native', text: '{}' }] }];
    daemon.emit(run(12, 'done', 'completed'));
    await settle();
    await settle();
    const messages = sync.getState().transcripts.s1.messages;
    expect(messages.map((m) => m.id)).toEqual(['m1', 'm2']);
    expect(messages[1].activity.map((a) => a.kind)).toEqual(['native']);
  });
});
