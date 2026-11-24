import type { Transport } from './types';

/**
 * Picks how this window reaches the daemon. The desktop shell is the only shipped path. Builds
 * made for UI tests (`VITE_TEST_BRIDGE=1`) may instead use the local test bridge named in the URL.
 */
export async function selectTransport(): Promise<Transport | null> {
  if ('__TAURI_INTERNALS__' in window) return (await import('./tauri')).tauriTransport();
  if (import.meta.env.VITE_TEST_BRIDGE === '1') {
    const query = new URLSearchParams(location.search);
    const url = query.get('bridge');
    const token = query.get('token');
    if (url && token) return (await import('./bridge')).bridgeTransport(url, token);
  }
  return null;
}

/** A window with no daemon link, such as a plain browser tab. */
export const unavailable: Transport = {
  call: () => Promise.reject('No Lumen service in this window'),
  subscribe: async () => () => {},
  connection: async () => ({ state: 'failed', detail: 'No Lumen service in this window' }),
};
