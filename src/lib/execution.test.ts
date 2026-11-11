import { describe, expect, it } from 'vitest';
import { executionState, type ExecutionTrace } from './execution';
import { recordActivity } from './activity';
import type { Activity, Message, Session } from './types';

const session: Session = {
  id: 's',
  title: 'Task',
  status: 'running',
  pinned: false,
  workspace: '/project',
  updatedAt: 1000,
};
const trace = (overrides: Partial<ExecutionTrace> = {}): ExecutionTrace => ({
  version: 1,
  id: 'call',
  provider: 'claude',
  phase: 'waiting-provider',
  startedAt: 1000,
  lastEventAt: 2000,
  processStartedAt: 1500,
  firstProviderEventAt: 2000,
  firstTextAt: null,
  endedAt: null,
  events: 1,
  toolEvents: 0,
  pendingTools: 0,
  pendingQuestions: 0,
  lastKind: 'initialized',
  timeline: [
    ['catalog', 1000],
    ['waiting-provider', 1500],
  ],
  ...overrides,
});
const activity = (kind: string, value: unknown): Activity => ({
  kind,
  text: JSON.stringify(value),
});
const messages = (rows: Activity[]): Message[] => [
  { id: 'user', role: 'user', text: 'Implement this', activity: [] },
  { id: 'assistant', role: 'assistant', text: 'I will implement this.', activity: rows },
];
describe('execution evidence', () => {
  it('selects a still-active provider after a newer parallel call has returned', () => {
    const rows = [
      activity('execution', trace({ id: 'slow' })),
      activity(
        'execution',
        trace({ id: 'fast', startedAt: 2500, endedAt: 3000, phase: 'returned' }),
      ),
      activity('execution-phase', { phase: 'turn-returned', at: 3100 }),
    ];
    expect(executionState(messages(rows), session, false)).toMatchObject({
      phase: 'waiting-provider',
      last: { id: 'slow' },
      silent: true,
    });
    rows.push(activity('execution', trace({ id: 'another', startedAt: 3200 })));
    expect(executionState(messages(rows), session, false).phase).toBe('parallel');
  });
  it('keeps crash-recovery timing finite and marks the unknown end time', () => {
    const state = executionState(
      messages([activity('execution', trace())]),
      { ...session, status: 'interrupted' },
      false,
    );
    expect(state).toMatchObject({
      endedAt: 2000,
      durationIncomplete: true,
      phase: 'turn-interrupted',
    });
  });
  it('bounds telemetry and lifecycle pressure while retaining task metadata', () => {
    let rows = [
      activity('checkpoint', 'tree'),
      activity('interaction', { id: 'q' }),
      activity('native', { id: 'native' }),
    ];
    for (let i = 0; i < 350; i++) {
      rows = recordActivity(rows, activity('tool-start', { id: `tool-${i}` }));
      rows = recordActivity(rows, activity('tool-end', { id: `tool-${i}` }));
      rows = recordActivity(rows, activity('execution', trace({ id: `call-${i}` })));
    }
    expect(rows.length).toBeLessThanOrEqual(500);
    expect(rows.filter((a) => a.kind === 'execution').length).toBeLessThanOrEqual(64);
    expect(rows.slice(0, 3).map((a) => a.kind)).toEqual(['checkpoint', 'interaction', 'native']);
  });
  it('distinguishes silent providers, pending questions/permissions and terminal states', () => {
    const rows = [activity('execution', trace())];
    expect(executionState(messages(rows), session, false).silent).toBe(true);
    rows.push(activity('interaction', { id: 'question', questions: [] }));
    expect(executionState(messages(rows), session, false)).toMatchObject({
      phase: 'question',
      silent: false,
    });
    rows.push(activity('interaction-result', { id: 'question', answers: {} }));
    expect(executionState(messages(rows), session, true)).toMatchObject({
      phase: 'approval',
      silent: false,
    });
    expect(
      executionState(messages(rows), { ...session, status: 'interrupted' }, false),
    ).toMatchObject({ phase: 'turn-interrupted', silent: false });
    expect(executionState(messages(rows), { ...session, status: 'error' }, false).phase).toBe(
      'turn-error',
    );
  });
  it('does not turn a promise-only answer or output-only child into completed work or an active agent', () => {
    const rows = [
      activity('execution', trace({ phase: 'returned', endedAt: 3000 })),
      activity('child', { id: 'child', kind: 'message', text: 'Output without lifecycle' }),
    ];
    expect(
      executionState(messages(rows), { ...session, status: 'completed' }, false),
    ).toMatchObject({ phase: 'turn-returned', agents: 0, silent: false });
    rows.push(activity('child', { id: 'child', kind: 'status', status: 'running' }));
    expect(executionState(messages(rows), session, false).agents).toBe(1);
    expect(executionState(messages(rows), { ...session, status: 'completed' }, false).agents).toBe(
      0,
    );
  });
  it('selects actual validation after a provider returns and preserves latest check status', () => {
    const rows = [
      activity('execution', trace({ endedAt: 3000, phase: 'returned' })),
      activity('execution-phase', { phase: 'verification', at: 4000 }),
      activity('validation', { workspace: '/project', label: 'Typecheck', status: 'failed' }),
      activity('validation', { workspace: '/project', label: 'Typecheck', status: 'passed' }),
    ];
    expect(executionState(messages(rows), session, false)).toMatchObject({
      phase: 'verification',
      checks: ['passed'],
      silent: false,
    });
  });
  it('scopes evidence to the latest turn and ignores malformed/unsupported telemetry', () => {
    const history = messages([activity('execution', trace())]);
    history.push({ id: 'next', role: 'user', text: 'Next task', activity: [] });
    history.push({
      id: 'reply',
      role: 'assistant',
      text: '',
      activity: [
        activity('execution', {
          version: 1,
          id: 'bad',
          phase: 'response',
          events: 1,
          startedAt: 1,
          timeline: [null],
        }),
      ],
    });
    expect(executionState(history, session, false)).toMatchObject({
      phase: 'waiting-start',
      stages: [],
      checks: [],
    });
  });
  it('coalesces streamed snapshots without evicting questions or checkpoints', () => {
    let rows = [activity('checkpoint', 'tree'), activity('interaction', { id: 'q' })];
    for (let i = 0; i < 2000; i++)
      rows = recordActivity(rows, activity('execution', trace({ events: i })));
    expect(rows).toHaveLength(3);
    expect(JSON.parse(rows[2].text).events).toBe(1999);
  });
});
