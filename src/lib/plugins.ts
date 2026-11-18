// Plugin contributions as the interface sees them. Pure: no Tauri access, so the panel rules
// below are testable in Node and in a real browser.

export interface PluginTheme {
  plugin: string;
  id: string;
  name: string;
  tokens: Record<string, Record<string, string>>;
}
export interface PluginCommand {
  plugin: string;
  name: string;
  description: string;
  prompt: string;
}
export interface PluginPanel {
  plugin: string;
  id: string;
  title: string;
  network: string[];
}
export interface Contributions {
  themes: PluginTheme[];
  slashCommands: PluginCommand[];
  panels: PluginPanel[];
}
export const NO_CONTRIBUTIONS: Contributions = { themes: [], slashCommands: [], panels: [] };
export const PLUGINS_CHANGED = 'lumen:plugins-changed';
export const INSERT_PROMPT = 'lumen:insert-prompt';
export const THEME_KEY = 'lumen-plugin-theme';

/** No allow-same-origin: the panel gets an opaque origin and can never reach Lumen's DOM or IPC. */
export const PANEL_SANDBOX = 'allow-scripts';
/** Kept identical to `PANEL_CSP` in src-tauri/src/lib.rs (a test checks it). */
export const PANEL_CSP =
  "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; form-action 'none'; sandbox allow-scripts";

export function panelUrl(
  plugin: string,
  panel: string,
  windows = /Windows/.test(navigator.userAgent),
) {
  const path = `${encodeURIComponent(plugin)}/${encodeURIComponent(panel)}`;
  return windows ? `http://lumen-plugin.localhost/${path}` : `lumen-plugin://localhost/${path}`;
}

const TOKEN = /^--[a-z0-9][a-z0-9-]{0,63}$/;
const VALUE = /^[^{}<>\\@;"']{1,200}$/;
const SELECTOR: Record<string, string> = {
  any: ':root',
  light: ":root[data-theme='light']",
  dark: ":root[data-theme='dark']",
};
/** Tokens only; anything that slipped past the backend check is dropped, not rendered. */
export function themeCss(theme?: PluginTheme): string {
  if (!theme) return '';
  return Object.entries(theme.tokens)
    .filter(([scheme]) => scheme in SELECTOR)
    .map(([scheme, tokens]) => {
      const body = Object.entries(tokens)
        .filter(([k, v]) => TOKEN.test(k) && VALUE.test(v) && !/url\(|expression/i.test(v))
        .map(([k, v]) => `${k}:${v};`)
        .join('');
      // `html` raises specificity above the built-in palette without !important.
      return `html${SELECTOR[scheme]}{${body}}`;
    })
    .join('\n');
}

export interface PanelBroker {
  insertPrompt(text: string): void;
  fetch(url: string): Promise<unknown>;
}
export type PanelReply = { lumen: 1; id: unknown; result?: unknown; error?: string };

/** The whole panel API: two methods. Everything else is refused. */
export async function handlePanelMessage(
  data: unknown,
  panel: PluginPanel,
  broker: PanelBroker,
): Promise<PanelReply | null> {
  if (!data || typeof data !== 'object' || (data as { lumen?: unknown }).lumen !== 1) return null;
  const { id, method, params } = data as { id?: unknown; method?: unknown; params?: unknown };
  const reply = (body: Omit<PanelReply, 'lumen' | 'id'>): PanelReply => ({ lumen: 1, id, ...body });
  const arg = (params && typeof params === 'object' ? params : {}) as Record<string, unknown>;
  try {
    if (method === 'prompt.insert') {
      if (typeof arg.text !== 'string' || !arg.text.trim() || arg.text.length > 8000)
        return reply({ error: 'prompt.insert needs text (1-8000 characters)' });
      broker.insertPrompt(arg.text);
      return reply({ result: true });
    }
    if (method === 'net.fetch') {
      if (!panel.network.length)
        return reply({ error: 'This plugin did not declare network access' });
      if (typeof arg.url !== 'string') return reply({ error: 'net.fetch needs a url' });
      return reply({ result: await broker.fetch(arg.url) });
    }
    return reply({ error: `Method not available to plugins: ${String(method)}` });
  } catch (e) {
    return reply({ error: e instanceof Error ? e.message : String(e) });
  }
}
