import type { Activity, Interaction, InteractionResult, Message } from '../../protocol/types';

/** How one activity kind appears in a message. Unknown kinds become generic steps. */
export type Role = 'step' | 'reasoning' | 'error' | 'notice' | 'image' | 'attachments' | 'model' | 'plan' | 'question' | 'hidden' | 'later';

// Telemetry and state the client reads elsewhere (or not at all), never listed as steps.
const HIDDEN = new Set([
  'native-usage', 'workspace-root', 'checkpoint', 'lane', 'worktree', 'applied', 'discarded', 'usage',
  'references', 'native', 'native-hooks', 'resume', 'run-options', 'execution', 'execution-phase',
  'latency', 'provider-event', 'pid', 'session', 'auto-assignment', 'tool-end', 'class', 'route',
  'code-metrics', 'start', 'done', 'status', 'workspace', 'task-worktree', 'tool-start', 'window',
  // Shown with the question it answers; quota rows feed the recovery card above the composer.
  'interaction-result', 'quota-reset', 'quota-scheduled',
]);
// Rendered by later milestones: Orchestra (M7), quota and cost (M6),
// memory and recipes (M8). Until then they stay out of the step list rather than show raw JSON.
const LATER = new Set([
  'connector-status', 'orchestra', 'orchestra-nogit', 'child',
  'quota', 'api-cost', 'memory-saved', 'memory-global',
  'memory-proposed', 'memory-held', 'recipe-saved', 'routine-created', 'audit', 'validation',
  'adapter-error',
]);

export function roleOf(kind: string): Role {
  if (kind === 'thinking') return 'reasoning';
  if (kind === 'error') return 'error';
  if (kind === 'autopilot-stopped') return 'notice';
  if (kind === 'image') return 'image';
  if (kind === 'attachments') return 'attachments';
  if (kind === 'model') return 'model';
  if (kind === 'plan') return 'plan';
  if (kind === 'interaction') return 'question';
  if (HIDDEN.has(kind)) return 'hidden';
  if (LATER.has(kind)) return 'later';
  return 'step';
}

export interface MessageParts {
  answer: string;
  reasoning: string;
  steps: Activity[];
  errors: string[];
  /** Why Autopilot stopped on its own terms: blocked, stalled or limit. Not an error. */
  notices: string[];
  images: string[];
  attachments: string[];
  model?: string;
  /** Why the daemon chose this agent for the reply. */
  why?: string;
  /** Questions the agent asked in this reply, with what was answered (secret answers are not kept). */
  asked: { request: Interaction; result?: InteractionResult }[];
}

const THINK = /<(think|thinking)>/i;

/** Reasoning some local models write inline as <think>…</think>; an open block is still reasoning. */
export function splitReasoning(text: string): { reasoning: string; answer: string } {
  let reasoning = '';
  let answer = '';
  let rest = text;
  for (;;) {
    const open = THINK.exec(rest);
    if (!open) return { reasoning, answer: answer + rest };
    answer += rest.slice(0, open.index);
    const after = rest.slice(open.index + open[0].length);
    const close = after.toLowerCase().indexOf(`</${open[1].toLowerCase()}>`);
    if (close < 0) return { reasoning: reasoning + after, answer };
    reasoning += after.slice(0, close) + '\n\n';
    rest = after.slice(close + open[1].length + 3);
  }
}

// Daemon directives for memory and routines are handled by the daemon, not shown as text.
const DIRECTIVES = /<lumen-(remember|routine)[\s\S]*?(<\/lumen-(remember|routine)>|$)\n?/g;

export function partsOf(message: Message): MessageParts {
  const split = message.role === 'assistant' ? splitReasoning(message.text.replace(DIRECTIVES, '')) : { reasoning: '', answer: message.text };
  const parts: MessageParts = { answer: split.answer, reasoning: '', steps: [], errors: [], notices: [], images: [], attachments: [], asked: [] };
  const reasoning: string[] = [];
  for (const activity of message.activity) {
    switch (roleOf(activity.kind)) {
      case 'reasoning':
        reasoning.push(activity.text);
        break;
      case 'error':
        parts.errors.push(activity.text);
        break;
      case 'notice':
        parts.notices.push(parse(activity.text)?.reason ?? '');
        break;
      case 'image':
        parts.images.push(activity.text);
        break;
      case 'attachments':
        parts.attachments.push(...activity.text.split('\n').filter(Boolean));
        break;
      case 'model':
        parts.model = activity.text;
        break;
      case 'plan':
        // "strategy · effort reasoning · reason": the reason is what the reader wants.
        parts.why = activity.text.split(' · ').slice(2).join(' · ').trim() || activity.text;
        break;
      case 'step':
        parts.steps.push(activity);
        break;
      case 'question': {
        const request = parse(activity.text) as Interaction | undefined;
        if (!request?.id || !Array.isArray(request.questions)) break;
        const result = message.activity
          .filter((a) => a.kind === 'interaction-result')
          .map((a) => parse(a.text) as InteractionResult | undefined)
          .find((r) => r?.id === request.id);
        parts.asked.push({ request, result });
        break;
      }
    }
  }
  parts.reasoning = [...reasoning, split.reasoning].filter((r) => r.trim()).join('\n\n').trim();
  return parts;
}

/** Claude and others send a tool as its name, a newline and the JSON input. */
function named(text: string): { name: string; input: any } | undefined {
  const newline = text.indexOf('\n');
  if (newline <= 0 || newline > 80) return undefined;
  const input = parse(text.slice(newline + 1));
  return input && typeof input === 'object' ? { name: text.slice(0, newline).trim(), input } : undefined;
}

/** One line that names a step, from whatever shape the provider sent. */
export function stepTitle(activity: Activity): { label: string; detail: string } {
  const tool = named(activity.text);
  const value = tool?.input ?? parse(activity.text);
  const pick = (...keys: string[]) => {
    for (const key of keys) {
      const v = value?.[key];
      if (typeof v === 'string' && v.trim()) return v.trim();
    }
    return '';
  };
  if (activity.kind === 'file') {
    const files = filesOf(activity);
    return { label: activity.kind, detail: files.map((f) => f.path).join(', ') };
  }
  const target = value ? pick('command', 'file_path', 'path', 'url', 'pattern', 'query', 'name', 'tool', 'title', 'label', 'text') : '';
  const detail = tool ? [tool.name, target].filter(Boolean).join(' ') : value ? target : firstLine(activity.text);
  return { label: activity.kind, detail: detail.length > 160 ? detail.slice(0, 159) + '…' : detail };
}

/** File changes as the daemon records them: a JSON list of { path, kind? }. */
export function filesOf(activity: Activity): { path: string; kind?: string }[] {
  const tool = named(activity.text);
  if (tool) return typeof tool.input.file_path === 'string' ? [{ path: tool.input.file_path, kind: tool.name }] : [];
  const value = parse(activity.text);
  const list = Array.isArray(value) ? value : Array.isArray(value?.changes) ? value.changes : [];
  return list.flatMap((c: unknown) => {
    if (typeof c === 'string') return [{ path: c }];
    if (c && typeof c === 'object' && typeof (c as { path?: unknown }).path === 'string') return [c as { path: string; kind?: string }];
    return [];
  });
}

/** Text for the expanded step: pretty JSON when it is JSON, the text as sent otherwise. */
export function stepBody(activity: Activity): string {
  const tool = named(activity.text);
  if (tool) return `${tool.name}\n${JSON.stringify(tool.input, null, 2)}`;
  const value = parse(activity.text);
  return value === undefined ? activity.text : JSON.stringify(value, null, 2);
}

function parse(text: string): any {
  const t = text.trimStart();
  if (!t.startsWith('{') && !t.startsWith('[')) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

const firstLine = (text: string) => text.split('\n', 1)[0].trim();

/**
 * The start of a message as plain text, for its accessible name. Off-screen messages are kept out
 * of layout (content-visibility), and WebView2 then exposes them to screen readers by name only.
 */
export function opening(text: string, length = 120): string {
  const plain = text
    .replace(/```[\s\S]*?(```|$)/g, ' ')
    .replace(/[#>*_`~|[\]()-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return plain.length > length ? plain.slice(0, length - 1).trimEnd() + '…' : plain;
}
