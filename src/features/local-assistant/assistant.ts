import { buildPrompt, cleanOutput } from './prompts';
import {
  DEFAULT_CONFIG,
  FAILURE_MESSAGES,
  TransportError,
  type AssistantResult,
  type AssistantTask,
  type FailureReason,
  type LocalAssistantConfig,
  type OllamaTransport,
} from './types';

function fail(reason: FailureReason): AssistantResult {
  return { ok: false, reason, message: FAILURE_MESSAGES[reason] };
}

/**
 * Local-only text helper. It never throws, never retries elsewhere and has no
 * remote provider to fall back to: on any failure the caller keeps the manual flow.
 */
export class LocalAssistant {
  private config: LocalAssistantConfig;
  private readonly transport: OllamaTransport;

  constructor(config: LocalAssistantConfig, transport: OllamaTransport) {
    this.config = config;
    this.transport = transport;
  }

  configure(config: Partial<LocalAssistantConfig>) {
    this.config = { ...this.config, ...config };
  }

  get settings(): LocalAssistantConfig {
    return this.config;
  }

  /** Whether a title should be generated without the user asking. */
  shouldAutoTitle(chat: { title: string; titleEdited: boolean; messageCount: number }): boolean {
    const c = this.config;
    return (
      c.enabled &&
      c.autoTitles &&
      Boolean(c.model) &&
      !chat.titleEdited &&
      !chat.title.trim() &&
      chat.messageCount > 0
    );
  }

  title(conversation: string, signal?: AbortSignal) {
    return this.run('title', conversation, signal);
  }
  /** On request only; the result is a draft for the user to edit. */
  commitMessage(diff: string, signal?: AbortSignal) {
    return this.run('commit', diff, signal);
  }
  /** On request only; the result is a draft for the user to edit. */
  handoffSummary(conversation: string, signal?: AbortSignal) {
    return this.run('handoff', conversation, signal);
  }

  /** Installed models, or the reason they cannot be listed. */
  async installedModels(
    signal?: AbortSignal,
  ): Promise<
    { ok: true; models: string[] } | { ok: false; reason: FailureReason; message: string }
  > {
    if (!this.config.enabled)
      return { ok: false, reason: 'disabled', message: FAILURE_MESSAGES.disabled };
    const guard = guardSignal(this.config.timeoutMs, signal);
    try {
      return { ok: true, models: await this.transport.models(guard.signal) };
    } catch (e) {
      const f = classify(e, guard);
      return { ok: false, reason: f, message: FAILURE_MESSAGES[f] };
    } finally {
      guard.dispose();
    }
  }

  private async run(
    task: AssistantTask,
    input: string,
    signal?: AbortSignal,
  ): Promise<AssistantResult> {
    const c = this.config;
    if (!c.enabled) return fail('disabled');
    if (!c.model) return fail('no-model');
    if (!input.trim()) return fail('invalid-input');
    const guard = guardSignal(c.timeoutMs, signal);
    try {
      const raw = await this.transport.generate({
        model: c.model,
        prompt: buildPrompt(task, input),
        signal: guard.signal,
      });
      if (guard.timedOut()) return fail('timeout');
      if (guard.cancelled()) return fail('cancelled');
      const text = cleanOutput(task, raw);
      return text ? { ok: true, text, model: c.model } : fail('empty');
    } catch (e) {
      return fail(classify(e, guard));
    } finally {
      guard.dispose();
    }
  }
}

interface Guard {
  signal: AbortSignal;
  timedOut(): boolean;
  cancelled(): boolean;
  dispose(): void;
}

function guardSignal(timeoutMs: number, outer?: AbortSignal): Guard {
  const ctl = new AbortController();
  let timedOut = false;
  const timer = setTimeout(
    () => {
      timedOut = true;
      ctl.abort();
    },
    timeoutMs > 0 ? timeoutMs : DEFAULT_CONFIG.timeoutMs,
  );
  const onAbort = () => ctl.abort();
  if (outer?.aborted) ctl.abort();
  else outer?.addEventListener('abort', onAbort, { once: true });
  return {
    signal: ctl.signal,
    timedOut: () => timedOut,
    cancelled: () => ctl.signal.aborted && !timedOut,
    dispose: () => {
      clearTimeout(timer);
      outer?.removeEventListener('abort', onAbort);
    },
  };
}

function classify(e: unknown, g: Guard): FailureReason {
  if (g.timedOut()) return 'timeout';
  if (g.cancelled()) return 'cancelled';
  if (e instanceof TransportError) return e.kind;
  return 'unavailable';
}
