import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Connection, Frame } from '../protocol/types';
import type { Transport } from './types';

/** The desktop shell relays the daemon stream on one ordered channel (see `bridge.rs`). */
export function tauriTransport(): Transport {
  return {
    call: (command, args = {}) => invoke('daemon_call', { command, args }),
    subscribe: (listener) => listen<Frame>('lumen:event', (e) => listener(e.payload)),
    connection: () => invoke<Connection>('daemon_connection'),
    pickFolder: () => invoke<string | null>('pick_folder'),
    notify: (title, body) => void invoke('notify', { title, body }).catch(() => {}),
    // Custom schemes are served as http://<scheme>.localhost on Windows (WebView2).
    panelUrl: (plugin, panel) => `${navigator.userAgent.includes('Windows') ? 'http://lumen-plugin.localhost' : 'lumen-plugin://localhost'}/${encodeURIComponent(plugin)}/${encodeURIComponent(panel)}`,
  };
}
