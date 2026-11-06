import { describe, expect, it } from 'vitest';
import { splitReasoning, workPhase } from './reasoning';

describe('splitReasoning', () => {
  it('leaves plain answers untouched', () => {
    expect(splitReasoning('Hello')).toEqual({ reasoning: '', answer: 'Hello' });
  });
  it('moves closed think blocks out of the answer', () => {
    expect(splitReasoning('<think>plan steps</think>\n\nFinal answer')).toEqual({
      reasoning: 'plan steps',
      answer: 'Final answer',
    });
  });
  it('treats an unclosed block as reasoning while streaming', () => {
    expect(splitReasoning('Intro <think>still thinking')).toEqual({
      reasoning: 'still thinking',
      answer: 'Intro',
    });
  });
  it('handles several blocks and mixed tag case', () => {
    const r = splitReasoning('<THINKING>a</thinking>x<think>b</think>y');
    expect(r.reasoning).toBe('a\n\nb');
    expect(r.answer).toBe('xy');
  });
});

describe('workPhase', () => {
  it('labels work from the latest activity', () => {
    expect(workPhase([])).toBe('writing');
    expect(workPhase(['thinking'])).toBe('thinking');
    expect(workPhase(['thinking', 'tool'])).toBe('tool');
    expect(workPhase(['tool', 'file', 'delta'])).toBe('tool');
  });
});
