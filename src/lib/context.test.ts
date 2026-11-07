import { describe, expect, it } from 'vitest';
import {
  contextLevel,
  contextUse,
  contextWindow,
  estimateTokens,
  lastMeasurement,
} from './context';

describe('context indicator', () => {
  it('estimates four characters per token', () => {
    expect(estimateTokens('abcdefgh')).toBe(2);
  });
  it('picks a window per provider and honours 1M model ids', () => {
    expect(contextWindow('claude', 'claude-sonnet-5-5')).toBe(200_000);
    expect(contextWindow('claude', 'claude-opus-5-5[1m]')).toBe(1_000_000);
    expect(contextWindow('ollama', 'llama3.2')).toBe(8_192);
  });
  it('clamps the percentage and sets a tone', () => {
    expect(contextLevel(100_000, 200_000)).toMatchObject({ percent: 50, tone: 'medium' });
    expect(contextLevel(500_000, 200_000)).toMatchObject({ percent: 100, tone: 'high' });
    expect(contextLevel(10, 0).percent).toBe(0);
  });
  it('reads the newest measurement, including the older bare-number format', () => {
    const reply = (id: string, activity: { kind: string; text: string }[]) => ({
      id,
      role: 'assistant' as const,
      text: '',
      activity,
    });
    expect(lastMeasurement([])).toBeNull();
    expect(lastMeasurement([reply('a', [{ kind: 'usage', text: '1200' }])])).toEqual({
      latest: 1200,
      overhead: null,
      window: null,
    });
    const measured = '{"latest":30000,"overhead":24000,"window":1000000}';
    const messages = [
      reply('a', [{ kind: 'usage', text: measured }]),
      reply('b', [{ kind: 'text', text: 'no usage' }]),
    ];
    expect(lastMeasurement(messages)).toEqual({ latest: 30000, overhead: 24000, window: 1000000 });
    expect(lastMeasurement([reply('c', [{ kind: 'usage', text: 'bad' }])])).toBeNull();
  });

  it('adds the measured CLI overhead and uses the reported window', () => {
    const use = contextUse(5000, 1000, 200_000, { latest: 30000, overhead: 24000, window: 60_000 });
    expect(use).toMatchObject({ used: 30000, window: 60_000, percent: 50, tone: 'medium' });
    expect(use.source).toBe('measured');
    const estimate = contextUse(5000, 1000, 200_000, null);
    expect(estimate).toMatchObject({ used: 6000, window: 200_000, overhead: null });
    expect(estimate.source).toBe('estimated');
  });
});
