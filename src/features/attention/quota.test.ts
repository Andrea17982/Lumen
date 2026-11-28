import { describe, expect, it } from 'vitest';
import { en } from '../../i18n/en';
import { it as italian } from '../../i18n/it';
import { elsewhereLabel, quotaText, switchText } from './quota';

describe('quota wording', () => {
  const ask = { provider: 'gpt', account: 'ChatGPT Pro', toProvider: 'claude', toAccount: 'Claude Pro', model: 'Opus 5.5', effort: 'high', weaker: false } as const;

  it('asks the quota-fallback question in the app language', () => {
    expect(switchText(italian, ask)).toBe('Codex ha esaurito la quota. Inviare la stessa richiesta a Opus 5.5 · high (Claude Pro)?');
    expect(switchText(en, { ...ask, effort: '', weaker: true })).toBe(
      'Codex has run out of quota. Send the same request to Opus 5.5 · default effort (Claude Pro)? This task may need a stronger model, so quality may drop.',
    );
  });

  it('shows hidden characters in names the user acts on as escapes', () => {
    expect(switchText(en, { ...ask, toAccount: 'Claude‮Pro' })).toContain('Claude\\u202ePro');
    expect(elsewhereLabel(italian, { accountId: 'b', account: 'Claude Pro', provider: 'claude', model: 'opus', modelName: 'Opus​ 5.5', effort: '', weaker: false })).toBe('Continua con Opus\\u200b 5.5 · Claude Pro');
  });

  it('explains each stop reason and keeps the provider message apart', () => {
    expect(quotaText(italian, { provider: 'claude', reason: 'no-alternative', detail: '' })).toEqual({
      summary: 'Claude Code ha esaurito la quota. Nessun altro account disponibile può gestire questa richiesta.',
      detail: '',
    });
    expect(quotaText(en, { provider: 'claude', model: 'fable', reason: 'model-access', detail: 'You have hit your monthly spend limit.' }).summary).toMatch(/^Claude Code refused fable/);
  });
});
