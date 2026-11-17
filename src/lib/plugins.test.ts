import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { PANEL_CSP, PANEL_SANDBOX, handlePanelMessage, panelUrl, themeCss } from './plugins';

const panel = { plugin: 'demo', id: 'main', title: 'Demo', network: [] as string[] };
const broker = () => {
  const calls: string[] = [];
  return {
    calls,
    insertPrompt: (text: string) => calls.push(`insert:${text}`),
    fetch: async (url: string) => {
      calls.push(`fetch:${url}`);
      return { status: 200 };
    },
  };
};

describe('plugin panels', () => {
  it('are sandboxed without same-origin and share the backend CSP', () => {
    expect(PANEL_SANDBOX).toBe('allow-scripts');
    expect(PANEL_CSP).toContain("default-src 'none'");
    expect(PANEL_CSP).toContain('sandbox allow-scripts');
    expect(PANEL_CSP).not.toMatch(/allow-same-origin|connect-src/);
    const rust = readFileSync('src-tauri/src/lib.rs', 'utf8');
    expect(/pub const PANEL_CSP: &str =\s*"([^"]*)";/.exec(rust)?.[1]).toBe(PANEL_CSP);
    expect(panelUrl('demo', 'main', true)).toBe('http://lumen-plugin.localhost/demo/main');
    expect(panelUrl('demo', 'main', false)).toBe('lumen-plugin://localhost/demo/main');
  });

  it('expose only the narrow broker and refuse undeclared capabilities', async () => {
    const b = broker();
    expect(await handlePanelMessage({ hello: 1 }, panel, b)).toBeNull();
    expect(
      await handlePanelMessage(
        { lumen: 1, id: 1, method: 'prompt.insert', params: { text: 'Hi' } },
        panel,
        b,
      ),
    ).toEqual({ lumen: 1, id: 1, result: true });
    for (const method of ['invoke', 'fs.read', 'shell.run', '__proto__']) {
      const reply = await handlePanelMessage({ lumen: 1, id: 2, method }, panel, b);
      expect(reply?.error).toMatch(/not available/);
    }
    const net = await handlePanelMessage(
      { lumen: 1, id: 3, method: 'net.fetch', params: { url: 'https://evil.test' } },
      panel,
      b,
    );
    expect(net?.error).toMatch(/did not declare network/);
    expect(b.calls).toEqual(['insert:Hi']);
    const declared = { ...panel, network: ['api.example.com'] };
    await handlePanelMessage(
      { lumen: 1, id: 4, method: 'net.fetch', params: { url: 'https://api.example.com/' } },
      declared,
      b,
    );
    expect(b.calls).toContain('fetch:https://api.example.com/');
  });
});

describe('plugin themes', () => {
  it('render tokens only and drop anything else', () => {
    const css = themeCss({
      plugin: 'p',
      id: 't',
      name: 'T',
      tokens: {
        any: { '--accent': '#123456', color: 'red', '--bg': 'url(https://x)' },
        dark: { '--text': '#fff' },
        body: { '--x': '1' },
      },
    });
    expect(css).toBe("html:root{--accent:#123456;}\nhtml:root[data-theme='dark']{--text:#fff;}");
    expect(themeCss(undefined)).toBe('');
  });
});
