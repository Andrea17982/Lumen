import { describe, expect, it } from 'vitest';
import type { ApprovalRequest, ApprovalSession, Interaction, Message } from '../../protocol/types';
import { blocker, collect, complete, countdown, openQuestions, recovery, shown, visibleText } from './requests';

const scope = { sessionId: 's1', connectionId: 'c1', provider: 'claude' as const, providerSessionId: 'p', turnId: 't' };
const request = (patch: Partial<ApprovalRequest> = {}): ApprovalRequest => ({
  id: 'r1',
  scope,
  providerItemId: null,
  kind: 'command',
  tool: 'Bash',
  command: 'git status',
  cwd: 'C:/w',
  reason: null,
  diffs: [],
  input: {},
  canApprove: true,
  unavailableReason: null,
  whitelistEligible: false,
  createdAtMs: 0,
  expiresAtMs: 300_000,
  status: 'pending',
  delivery: 'none',
  decision: null,
  decisionSource: null,
  note: null,
  ...patch,
});
const live: ApprovalSession = { scope, cwd: 'C:/w', availability: { state: 'available', version: '1', evidence: '' } };

describe('approvals', () => {
  it('can be decided only while pending, unexpired and connected', () => {
    expect(blocker(request(), [live], 1_000)).toBeNull();
    expect(blocker(request(), [live], 300_000)).toBe('expired');
    expect(blocker(request(), [], 1_000)).toBe('disconnected');
    expect(blocker(request({ worker: { agent: 'Codex', taskId: 'a', task: 'Tests' } }), [{ ...live, availability: { state: 'unavailable', reason: 'gone' } }], 1_000)).toBe('workerGone');
    expect(blocker(request({ scope: { ...scope, turnId: 'older' } }), [live], 1_000)).toBe('disconnected');
    expect(blocker(request({ status: 'approved' }), [], 1_000)).toBeNull();
  });

  it('shows open, uncertain and recently seen requests, not the journal history', () => {
    const all = [request({ id: 'open' }), request({ id: 'old', status: 'approved' }), request({ id: 'seen', status: 'timedOut' }), request({ id: 'lost', status: 'approved', delivery: 'uncertain' })];
    expect(shown(all, new Set(['seen']), 's1').map((r) => r.id)).toEqual(['open', 'seen', 'lost']);
    expect(shown(all, new Set(), 'other')).toEqual([]);
  });

  it('counts down in minutes and seconds, never below zero', () => {
    expect(countdown(299_001)).toBe('5:00');
    expect(countdown(61_000)).toBe('1:01');
    expect(countdown(-5)).toBe('0:00');
  });

  it('shows hidden and direction-changing characters as escapes', () => {
    expect(visibleText('rm -rf safe\u202e/ txt\u200b')).toBe('rm -rf safe\\u202e/ txt\\u200b');
    expect(visibleText('line\nnext\ttab')).toBe('line\nnext\ttab');
    // Bidi marks, soft hyphens, blank-drawing and space lookalikes, separators, variation selectors, tags.
    for (const code of [0x061c, 0x00ad, 0x3164, 0x00a0, 0x2028, 0xfe0f, 0x202e, 0xe0041]) {
      const hex = code.toString(16);
      const escape = String.fromCharCode(92) + (code > 0xffff ? `u{${hex}}` : `u${hex.padStart(4, '0')}`);
      expect(visibleText(`a${String.fromCodePoint(code)}b`)).toBe(`a${escape}b`);
    }
    expect(visibleText('plain ASCII and accents: è ü 日本')).toBe('plain ASCII and accents: è ü 日本');
  });
});

const ask: Interaction = {
  id: 'q1',
  questions: [
    { id: 'impl', header: 'Implementation', question: 'Which one?', options: [{ label: 'Native', description: '' }, { label: 'Custom', description: '' }], multiSelect: false, secret: false, optional: false },
    { id: 'token', header: '', question: 'Token?', options: [], multiSelect: false, secret: true, optional: false },
    { id: 'notes', header: '', question: 'Anything else?', options: [], multiSelect: false, secret: false, optional: true },
  ],
};
const message = (id: string, activity: { kind: string; text: string }[]): Message => ({ id, role: 'assistant', text: '', activity, eventSeq: 0 });

describe('questions', () => {
  it('are open only while the daemon still waits and no result was recorded', () => {
    const asked = message('m2', [{ kind: 'interaction', text: JSON.stringify(ask) }]);
    expect(openQuestions([asked], ['q1']).map((q) => q.state)).toEqual(['open']);
    expect(openQuestions([asked], [])).toEqual([]);
    expect(openQuestions([asked], null).map((q) => q.state)).toEqual(['unknown']);
    const answered = message('m2', [...asked.activity, { kind: 'interaction-result', text: JSON.stringify({ id: 'q1', answers: {} }) }]);
    expect(openQuestions([answered], ['q1'])).toEqual([]);
    expect(openQuestions([message('m2', [{ kind: 'interaction', text: 'not json' }])], ['q1'])).toEqual([]);
  });

  it('ignore questions asked in older turns', () => {
    const old = message('m0', [{ kind: 'interaction', text: JSON.stringify(ask) }]);
    const later = ['a', 'b', 'c', 'd'].map((id) => message(id, []));
    expect(openQuestions([old, ...later], ['q1'])).toEqual([]);
  });

  it('send a typed answer instead of a single choice, and need every required answer', () => {
    const answers = collect(ask, { impl: ['Native'] }, { impl: 'Something else', token: ' s3cret ' });
    expect(answers).toEqual({ impl: ['Something else'], token: ['s3cret'], notes: [] });
    expect(complete(ask, answers)).toBe(true);
    expect(complete(ask, collect(ask, { impl: ['Native'] }, {}))).toBe(false);
  });

  it('let a declined connector request go without its form', () => {
    const connector: Interaction = {
      id: 'c',
      kind: 'connector',
      actionId: 'action',
      questions: [
        { id: 'action', header: 'Offline GitHub', question: 'Review\nAllow access?', options: [{ label: 'Accept', description: '' }, { label: 'Decline', description: '' }], multiSelect: false, secret: false, optional: false },
        { id: 'count', header: '', question: 'Count', options: [], multiSelect: false, secret: false, optional: false },
      ],
    };
    expect(complete(connector, { action: ['Decline'], count: [] })).toBe(true);
    expect(complete(connector, { action: ['Accept'], count: [] })).toBe(false);
  });
});

describe('recovery', () => {
  const stopped = (activity: { kind: string; text: string }[]) => [message('m2', activity)];
  const quota = [
    { kind: 'native', text: '{}' },
    { kind: 'quota-reset', text: JSON.stringify({ at: 1_800_000, accountId: 'a' }) },
    { kind: 'error', text: 'You have hit your usage limit' },
  ];

  it('offers one resume at the quota reset only for a stopped reply with a native session', () => {
    expect(recovery(stopped(quota), 'error')).toEqual({ kind: 'quota', messageId: 'm2', resume: { at: 1_800_000, estimated: false }, elsewhere: null });
    expect(recovery(stopped(quota.filter((a) => a.kind !== 'native')), 'error')).toBeNull();
    expect(recovery(stopped(quota), 'running')).toBeNull();
  });

  it('shows a scheduled resume while waiting, and nothing more once it resumes', () => {
    const waiting = [...quota, { kind: 'quota-scheduled', text: JSON.stringify({ at: 1_800_000, state: 'waiting' }) }];
    expect(recovery(stopped(waiting), 'waiting')).toEqual({ kind: 'scheduled', at: 1_800_000 });
    const resumed = [...waiting, { kind: 'quota-scheduled', text: JSON.stringify({ at: 1_800_000, state: 'resuming' }) }];
    expect(recovery(stopped(resumed), 'completed')).toBeNull();
    const cancelled = [...waiting, { kind: 'quota-scheduled', text: JSON.stringify({ at: 1_800_000, state: 'cancelled' }) }];
    expect(recovery(stopped(cancelled), 'interrupted')).toEqual({ kind: 'quota', messageId: 'm2', resume: { at: 1_800_000, estimated: false }, elsewhere: null });
  });

  it('offers the proposed equivalent model elsewhere, with or without a native session', () => {
    const proposal = { accountId: 'b', account: 'Claude', provider: 'claude', model: 'opus', modelName: 'Opus 5.5', effort: 'high', weaker: false };
    const elsewhere = [...quota.filter((a) => a.kind !== 'native'), { kind: 'quota-alternative', text: JSON.stringify(proposal) }];
    expect(recovery(stopped(elsewhere), 'error')).toEqual({ kind: 'quota', messageId: 'm2', resume: null, elsewhere: proposal });
    const both = recovery(stopped([...quota, { kind: 'quota-alternative', text: JSON.stringify(proposal) }]), 'error');
    expect(both).toMatchObject({ kind: 'quota', resume: { at: 1_800_000 }, elsewhere: proposal });
    expect(recovery(stopped(elsewhere), 'running')).toBeNull();
    // A finished reply offers nothing, and a partial proposal row never names an action.
    expect(recovery(stopped(elsewhere), 'completed')).toBeNull();
    const partial = [...quota.filter((a) => a.kind !== 'native'), { kind: 'quota-alternative', text: JSON.stringify({ accountId: 'b', model: 'opus' }) }];
    expect(recovery(stopped(partial), 'error')).toBeNull();
  });

  it('marks a reset time Lumen had to estimate', () => {
    const estimated = quota.map((a) => (a.kind === 'quota-reset' ? { ...a, text: JSON.stringify({ at: 1_800_000, estimated: true }) } : a));
    expect(recovery(stopped(estimated), 'error')).toMatchObject({ resume: { at: 1_800_000, estimated: true } });
  });

  it('says an interrupted reply was kept and not continued', () => {
    expect(recovery(stopped([]), 'interrupted')).toEqual({ kind: 'interrupted' });
    expect(recovery(stopped([]), 'completed')).toBeNull();
    expect(recovery([{ ...message('m1', []), role: 'user' }], 'interrupted')).toBeNull();
  });
});
