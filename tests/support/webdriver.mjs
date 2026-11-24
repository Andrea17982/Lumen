// Minimal W3C WebDriver client for the desktop app test: just the calls it needs, over fetch.
import { sleep } from '../daemon-harness.mjs';

const ELEMENT = 'element-6066-11e4-a52e-4f735466cecf';

export async function webdriver(port, { timeout = 30_000 } = {}) {
  const base = `http://127.0.0.1:${port}`;
  async function request(method, path, body) {
    const response = await fetch(base + path, {
      method,
      headers: { 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const { value } = await response.json();
    if (!response.ok) throw new Error(`${method} ${path}: ${value?.error ?? response.status} ${value?.message ?? ''}`);
    return value;
  }
  // The app opens its server a moment after the process starts.
  let id;
  for (const start = Date.now(); !id; ) {
    try {
      id = (await request('POST', '/session', { capabilities: { alwaysMatch: {} } })).sessionId;
    } catch (error) {
      if (Date.now() - start > timeout) throw error;
      await sleep(250);
    }
  }
  const session = (path) => `/session/${id}${path}`;
  const run = (script, ...args) => request('POST', session('/execute/sync'), { script, args });
  return {
    run,
    async click(css) {
      const element = await request('POST', session('/element'), { using: 'css selector', value: css });
      await request('POST', session(`/element/${element[ELEMENT]}/click`), {});
    },
    /** Polls a page expression until `test` accepts its value. */
    async until(script, test, label, wait = timeout) {
      let last;
      for (const start = Date.now(); Date.now() - start < wait; await sleep(200)) {
        last = await run(script).catch((error) => `error: ${error.message}`);
        if (test(last)) return last;
      }
      throw new Error(`Timed out waiting for ${label}; last value: ${JSON.stringify(last)}`);
    },
    close: () => request('DELETE', session('')).catch(() => {}),
  };
}
