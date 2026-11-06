export type AssistantTask = 'title' | 'commit' | 'handoff';

export interface LocalAssistantConfig {
  /** Master switch; off by default. When off nothing is ever generated. */
  enabled: boolean;
  /** Installed Ollama model; empty means "none chosen yet". */
  model: string;
  /** Only titles may run unprompted. Commit messages and handoffs never do. */
  autoTitles: boolean;
  timeoutMs: number;
}

export const DEFAULT_CONFIG: LocalAssistantConfig = {
  enabled: false,
  model: '',
  autoTitles: true,
  timeoutMs: 60_000,
};

export interface GenerateRequest {
  model: string;
  prompt: string;
  signal: AbortSignal;
}

/** Everything the assistant needs from Ollama. Implementations must stay local. */
export interface OllamaTransport {
  generate(req: GenerateRequest): Promise<string>;
  /** Names of installed models; rejects when the service is unreachable. */
  models(signal: AbortSignal): Promise<string[]>;
}

export type FailureReason =
  | 'disabled'
  | 'no-model'
  | 'unavailable'
  | 'model-missing'
  | 'timeout'
  | 'cancelled'
  | 'empty'
  | 'invalid-input';

export type AssistantResult =
  { ok: true; text: string; model: string } | { ok: false; reason: FailureReason; message: string };

/** Thrown by transports so the assistant can classify failures. */
export class TransportError extends Error {
  readonly kind: 'unavailable' | 'model-missing';
  constructor(kind: 'unavailable' | 'model-missing', message: string) {
    super(message);
    this.kind = kind;
  }
}

export const FAILURE_MESSAGES: Record<FailureReason, string> = {
  disabled: 'The local assistant is turned off.',
  'no-model': 'Choose a local model in Settings first.',
  unavailable: 'Ollama is not reachable. Open the Ollama app, or write this by hand.',
  'model-missing': 'The selected model is not installed. Pick another or run `ollama pull`.',
  timeout: 'Ollama took too long. Try again, or write this by hand.',
  cancelled: 'Cancelled.',
  empty: 'The model returned nothing usable. Try again, or write this by hand.',
  'invalid-input': 'There is nothing to summarize yet.',
};
