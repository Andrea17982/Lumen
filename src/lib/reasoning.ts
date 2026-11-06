export interface SplitReply {
  /** Reasoning embedded in the reply as <think>…</think> blocks (some local models do this). */
  reasoning: string;
  /** What the user reads as the answer. */
  answer: string;
}

const OPEN = /<(think|thinking)>/i;

/**
 * Separates inline reasoning from the answer. An unclosed block (still streaming)
 * counts as reasoning, so half-written thoughts never appear as answer text.
 */
export function splitReasoning(text: string): SplitReply {
  let reasoning = '';
  let answer = '';
  let rest = text;
  for (;;) {
    const open = OPEN.exec(rest);
    if (!open) {
      answer += rest;
      break;
    }
    answer += rest.slice(0, open.index);
    const tag = open[1].toLowerCase();
    const after = rest.slice(open.index + open[0].length);
    const close = after.toLowerCase().indexOf(`</${tag}>`);
    if (close === -1) {
      reasoning += after;
      break;
    }
    reasoning += (reasoning ? '\n\n' : '') + after.slice(0, close);
    rest = after.slice(close + tag.length + 3);
  }
  return { reasoning: reasoning.trim(), answer: answer.trim() };
}

export type WorkPhase = 'writing' | 'thinking' | 'tool';

/** The most recent kind of work, used to label the in-progress indicator. */
export function workPhase(kinds: string[]): WorkPhase {
  const last = [...kinds].reverse().find((k) => ['thinking', 'tool', 'file'].includes(k));
  if (last === 'thinking') return 'thinking';
  if (last === 'tool' || last === 'file') return 'tool';
  return 'writing';
}
