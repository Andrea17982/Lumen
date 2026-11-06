import { TransportError, type GenerateRequest, type OllamaTransport } from './types';
import { call } from '../../lib/bridge';

export const DEFAULT_OLLAMA_URL = 'http://127.0.0.1:11434';

const LOOPBACK = new Set(['127.0.0.1', 'localhost', '[::1]', '::1']);

/** The helper is local-only: any non-loopback endpoint is refused outright. */
export function assertLoopback(url: string): URL {
  const u = new URL(url);
  if (u.protocol !== 'http:' || !LOOPBACK.has(u.hostname)) {
    throw new Error(`Local assistant only talks to a loopback Ollama, not ${u.host}.`);
  }
  return u;
}

/** Talks to the local Ollama daemon over loopback HTTP. */
export function httpTransport(
  baseUrl = DEFAULT_OLLAMA_URL,
  fetchImpl: typeof fetch = (...a) => fetch(...a),
): OllamaTransport {
  const base = assertLoopback(baseUrl);
  const call = async (path: string, init: RequestInit) => {
    try {
      return await fetchImpl(new URL(path, base).toString(), { ...init, redirect: 'error' });
    } catch (e) {
      if ((e as Error)?.name === 'AbortError' || init.signal?.aborted) throw e;
      throw new TransportError('unavailable', 'Ollama is not reachable.');
    }
  };
  return {
    async generate({ model, prompt, signal }: GenerateRequest) {
      const res = await call('/api/generate', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ model, prompt, stream: false, options: { temperature: 0.3 } }),
        signal,
      });
      if (res.status === 404)
        throw new TransportError('model-missing', `Model ${model} not found.`);
      if (!res.ok) throw new TransportError('unavailable', `Ollama answered ${res.status}.`);
      const data = (await res.json()) as { response?: string };
      return data.response ?? '';
    },
    async models(signal) {
      const res = await call('/api/tags', { signal });
      if (!res.ok) throw new TransportError('unavailable', `Ollama answered ${res.status}.`);
      const data = (await res.json()) as { models?: { name: string }[] };
      return (data.models ?? []).map((m) => m.name);
    },
  };
}

/**
 * The app's transport: the webview cannot reach the loopback daemon (CSP), so the
 * requests go through native commands that run the official `ollama` CLI.
 */
export function ollamaTransport(): OllamaTransport {
  const fail = (e: unknown): never => {
    const text = String(e instanceof Error ? e.message : e);
    if (text.startsWith('model-missing:')) throw new TransportError('model-missing', text);
    throw new TransportError('unavailable', text);
  };
  return {
    async generate({ model, prompt, signal }: GenerateRequest) {
      if (signal.aborted) throw new DOMException('Aborted', 'AbortError');
      // The native side enforces the timeout; the assistant's guard stops waiting on its own.
      return call<string>('ollama_generate', { model, prompt }).catch(fail);
    },
    async models() {
      return call<string[]>('ollama_models').catch(fail);
    },
  };
}
