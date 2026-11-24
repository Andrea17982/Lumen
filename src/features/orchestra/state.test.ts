import { describe, expect, it } from 'vitest';
import type { Message } from '../../protocol/types';
import { orchestraOf, overLimit, planEdits, tail, type Subtask } from './state';

const message = (text: string): Message => ({ id: 'm', role: 'assistant', text: '', activity: [{ kind: 'orchestra', text }], eventSeq: 0 });
const task = (id: string, accountId = ''): Subtask => ({ id, title: id, detail: '', files: [], difficulty: 'normal', writes: true, dependsOn: [], accountId, agent: '', model: '', effort: '', status: 'pending', output: '' });

describe('orchestra', () => {
  it('reads the published state and ignores anything else', () => {
    expect(orchestraOf(message('{"stage":"plan","subtasks":[{"id":"s1"}]}'))?.subtasks).toHaveLength(1);
    expect(orchestraOf(message('not json'))).toBeNull();
    expect(orchestraOf(message('{"subtasks":[]}'))).toBeNull();
    expect(orchestraOf({ ...message(''), activity: [] })).toBeNull();
  });
  it('sends only the edits the user made', () => {
    const subtasks = [task('s1', 'a'), task('s2', 'a'), task('s3')];
    expect(planEdits(subtasks, { s1: 'a', s2: 'b', s3: '' }, new Set(['s3']))).toEqual([{ id: 's2', accountId: 'b' }, { id: 's3', remove: true }]);
    expect(planEdits(subtasks, { s1: '' }, new Set())).toEqual([]);
  });
  it('counts parts held back only by the total limit', () => {
    const state = orchestraOf(message('{"stage":"running","queue":{"waiting":[{"id":"s3","reason":"total-cap"},{"id":"s4","reason":"quota"}]}}'))!;
    expect(overLimit(state)).toBe(1);
  });
  it('keeps the end of a long lane', () => {
    const long = 'x'.repeat(10000) + '\nlast line';
    expect(tail(long).endsWith('last line')).toBe(true);
    expect(tail(long).length).toBeLessThanOrEqual(4002);
    expect(tail('short')).toBe('short');
  });
});
