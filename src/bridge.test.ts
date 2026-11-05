import { describe, expect, it } from 'vitest';
import { applyEvent } from './bridge';
import type { Message, RunEvent } from './types';
const e = (kind: string, text: string): RunEvent => ({
  sessionId: 's',
  messageId: 'm',
  kind,
  text,
});
describe('stream reconciliation', () => {
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
});
