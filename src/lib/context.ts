import type { Message, Provider } from './types';

/** Local token estimate (about four characters per token), never a provider-reported figure. */
export const tokensForChars = (chars: number) => Math.ceil(chars / 4);
export const estimateTokens = (text: string) => tokensForChars(text.length);

/**
 * Published context window by provider and model id, used until the CLI reports the real one
 * (Claude does on every reply). The official CLIs manage their own context and may differ.
 */
export function contextWindow(provider: Provider, model: string): number {
  const id = model.toLowerCase();
  if (/(^|[^0-9])1m\b/.test(id)) return 1_000_000;
  switch (provider) {
    case 'claude':
      return 200_000;
    case 'gpt':
      return 400_000;
    case 'gemini':
    case 'antigravity':
      return 1_000_000;
    case 'ollama':
      return 8_192;
  }
}

export interface ContextLevel {
  used: number;
  window: number;
  percent: number;
  tone: 'low' | 'medium' | 'high';
}

export function contextLevel(used: number, window: number): ContextLevel {
  const percent = window > 0 ? Math.min(100, (used / window) * 100) : 0;
  return { used, window, percent, tone: percent >= 80 ? 'high' : percent >= 50 ? 'medium' : 'low' };
}

/** What a provider reported for its latest turn in this chat. */
export interface Measurement {
  /** Input tokens of the latest request, cached reads and writes included. */
  latest: number;
  /** CLI instructions and tools on top of Lumen's prompt, measured on the turn's first request. */
  overhead: number | null;
  /** Context window the CLI reported for the model. */
  window: number | null;
}

const count = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) && v >= 0 ? v : null);

/** The newest reply's measurement. Older replies stored a bare number of input tokens. */
export function lastMeasurement(messages: Message[]): Measurement | null {
  for (let i = messages.length - 1; i >= 0; i--) {
    const usage = messages[i].activity.find((a) => a.kind === 'usage');
    if (!usage) continue;
    let value: unknown;
    try {
      value = JSON.parse(usage.text);
    } catch {
      return null;
    }
    if (count(value) !== null) return { latest: value as number, overhead: null, window: null };
    const v = (value ?? {}) as Record<string, unknown>;
    const latest = count(v.latest);
    return latest === null
      ? null
      : { latest, overhead: count(v.overhead), window: count(v.window) };
  }
  return null;
}

export interface ContextUse extends ContextLevel {
  prompt: number;
  draft: number;
  overhead: number | null;
  /** `measured` when the window and CLI overhead come from the provider's own report. */
  source: 'measured' | 'estimated';
  lastReported?: number;
}

/**
 * Next prompt size: Lumen's prompt and draft (local estimate) plus the CLI overhead the provider
 * measured on its last turn. The window comes from the provider's report when it has one.
 */
export function contextUse(
  prompt: number,
  draft: number,
  fallbackWindow: number,
  measurement: Measurement | null,
): ContextUse {
  const overhead = measurement?.overhead ?? null;
  const window = measurement?.window ?? fallbackWindow;
  return {
    ...contextLevel(prompt + draft + (overhead ?? 0), window),
    prompt,
    draft,
    overhead,
    source: overhead !== null && measurement?.window ? 'measured' : 'estimated',
    lastReported: measurement?.latest,
  };
}
