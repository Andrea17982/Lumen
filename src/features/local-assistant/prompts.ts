import type { AssistantTask } from './types';

const MAX_INPUT = 6000;

/** Keep the head and tail of long inputs; small local models choke on huge prompts. */
export function clip(text: string, max = MAX_INPUT): string {
  const t = text.trim();
  if (t.length <= max) return t;
  const half = Math.floor(max / 2);
  return `${t.slice(0, half)}\n[...]\n${t.slice(-half)}`;
}

export function buildPrompt(task: AssistantTask, input: string): string {
  const body = clip(input);
  switch (task) {
    case 'title':
      return `Write a title of at most 6 words for this conversation. Reply with the title only: no quotes, no punctuation at the end.\n\n${body}`;
    case 'commit':
      return `Write a git commit message for this diff. First line: imperative summary under 72 characters. Optionally a blank line then a short body. Reply with the message only, no code fences.\n\n${body}`;
    case 'handoff':
      return `Summarize this conversation so another AI assistant can continue it: goal, decisions made, current state, next steps. Be concise. Use short bullet points.\n\n${body}`;
  }
}

/** Strip the decoration small models like to add around an answer. */
export function cleanOutput(task: AssistantTask, raw: string): string {
  let t = raw
    .replace(/<think>[\s\S]*?<\/think>/gi, '')
    .replace(/^```[a-z]*\n?|\n?```$/gim, '')
    .trim();
  if (task === 'title') {
    t = (t.split('\n').find((l) => l.trim()) ?? '')
      .replace(/^(title|titolo)\s*:\s*/i, '')
      .replace(/^["'“”`*#\s]+|["'“”`*#\s.]+$/g, '')
      .trim();
    if (t.length > 80) t = t.slice(0, 80).trimEnd();
  }
  return t;
}
