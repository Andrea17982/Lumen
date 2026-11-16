import { describe, expect, it } from 'vitest';
import { applyEvent, mergeMessages } from './bridge';
import type { Message, RunEvent } from './types';
const e = (kind: string, text: string): RunEvent => ({
  sessionId: 's',
  messageId: 'm',
  kind,
  text,
});
describe('stream reconciliation', () => {
  it('coalesces nested agent streams without evicting pending questions or checkpoints', () => {
    let m = applyEvent([], e('checkpoint', 'base'));
    m = applyEvent(m, e('interaction', '{"id":"question"}'));
    for (let i = 0; i < 1200; i++)
      m = applyEvent(m, e('child', JSON.stringify({ id: 'agent', kind: 'delta', text: 'x' })));
    expect(m[0].activity).toHaveLength(3);
    expect(m[0].activity[0].text).toBe('base');
    expect(JSON.parse(m[0].activity[2].text).text).toHaveLength(1200);
    expect(m[0].text).toBe('');
  });
  it('creates an assistant, applies chunks and authoritative replacement without duplication', () => {
    let m: Message[] = applyEvent([], e('start', ''));
    m = applyEvent(m, e('delta', 'Hello'));
    m = applyEvent(m, e('delta', ' there'));
    m = applyEvent(m, e('replace', 'Hello there.'));
    expect(m).toHaveLength(1);
    expect(m[0].text).toBe('Hello there.');
  });
  it('retains provider diagnostics separately from assistant text', () => {
    const m = applyEvent([], e('error', 'Sign-in required'));
    expect(m[0].text).toBe('');
    expect(m[0].activity[0].text).toBe('Sign-in required');
  });
  it('replaces the Orchestra state in place and keeps live subtask output apart', () => {
    let m = applyEvent([], e('orchestra', '{"stage":"plan"}'));
    m = applyEvent(m, e('tool', 'Read a.rs'));
    m = applyEvent(m, e('orchestra', '{"stage":"running"}'));
    m = applyEvent(m, e('orchestra-delta', '{"id":"s1","kind":"delta","text":"Hel"}'));
    m = applyEvent(m, e('orchestra-delta', '{"id":"s1","kind":"delta","text":"lo"}'));
    m = applyEvent(m, e('orchestra-delta', 'not json'));
    expect(m[0].activity.filter((a) => a.kind === 'orchestra')).toEqual([
      { kind: 'orchestra', text: '{"stage":"running"}' },
    ]);
    expect(m[0].live).toEqual({ s1: 'Hello' });
    expect(m[0].text).toBe('');
  });
});

describe('service snapshot ordering', () => {
  it('does not turn a conversation-level broadcast completion into an empty assistant', () => {
    const messages: Message[] = [];
    expect(applyEvent(messages, { ...e('done', 'completed'), messageId: '' })).toBe(messages);
  });
  it('deduplicates events covered by a snapshot and preserves events received while it loads', () => {
    const snapshot: Message[] = [
      { id: 'm', role: 'assistant', text: 'Hello', activity: [], eventSeq: 10, snapshotSeq: 10 },
    ];
    expect(applyEvent(snapshot, { ...e('delta', 'Hello'), seq: 10 })).toBe(snapshot);
    const current = applyEvent(snapshot, { ...e('delta', ' world'), seq: 11 });
    expect(mergeMessages(snapshot, current)[0].text).toBe('Hello world');
    expect(applyEvent(current, { ...e('delta', ' world'), seq: 11 })).toBe(current);
  });
  it('does not restore discarded messages already covered by the snapshot watermark', () => {
    const snapshot: Message[] = [
      { id: 'user', role: 'user', text: 'Kept', activity: [], snapshotSeq: 50 },
    ];
    const previous = applyEvent([], { ...e('delta', 'Discarded'), seq: 20 });
    expect(mergeMessages(snapshot, previous)).toEqual(snapshot);
  });
});
