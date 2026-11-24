import type { Connection, Frame } from '../protocol/types';
import type { Transport } from './types';

/**
 * UI tests only: talks to `tests/support/ipc-bridge.mjs`, which relays the daemon on 127.0.0.1
 * behind a random token. Excluded from shipped builds by `select.ts`.
 */
export function bridgeTransport(url: string, token: string): Transport {
  const headers = { authorization: `Bearer ${token}`, 'content-type': 'application/json' };
  return {
    async call(command, args = {}) {
      const response = await fetch(`${url}/call`, { method: 'POST', headers, body: JSON.stringify({ command, args }) });
      const reply = (await response.json()) as { ok: unknown } | { error: string };
      if ('error' in reply) throw reply.error;
      return reply.ok;
    },
    subscribe(listener) {
      const source = new EventSource(`${url}/events?token=${encodeURIComponent(token)}`);
      source.onmessage = (e) => listener(JSON.parse(e.data) as Frame);
      return new Promise((resolve) => {
        source.onopen = () => resolve(() => source.close());
      });
    },
    // Tests answer the browser's prompt with a folder path.
    pickFolder: async () => prompt('Folder') || null,
    connection: async () => (await (await fetch(`${url}/connection`, { headers })).json()) as Connection,
  };
}
