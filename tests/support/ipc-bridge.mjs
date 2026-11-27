// UI tests only: relays a real daemon to a browser page over 127.0.0.1, behind a random token.
// It plays the desktop shell's part (src-tauri/desktop/src/bridge.rs) without starting the
// daemon: tests own its lifetime, so they can kill and restart it.
import { randomBytes } from 'node:crypto';
import http from 'node:http';
import { sleep } from '../daemon-harness.mjs';

// Copied from src-tauri/desktop/src/main.rs (PANEL_CSP).
const PANEL_CSP = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; form-action 'none'; base-uri 'none'; sandbox allow-scripts";

/** @param {Awaited<ReturnType<typeof import('../daemon-harness.mjs').daemonClient>>} client */
export async function startBridge(client, { origin }) {
  const token = randomBytes(24).toString('hex');
  const listeners = new Set();
  let connection = { state: 'connecting' };
  let closed = false;
  let stream;

  const send = (res, frame) => res.write(`data: ${JSON.stringify(frame)}\n\n`);
  const broadcast = (frame) => listeners.forEach((res) => send(res, frame));
  const setConnection = (next) => {
    if (next.state === connection.state) return;
    connection = next;
    broadcast({ event: 'lumen:connection', payload: next });
  };

  // Same policy as the shell: relay until the stream ends, then reconnect without replaying.
  (async () => {
    let connectedOnce = false;
    while (!closed) {
      try {
        stream = await client.open('__events');
        const ready = await stream.next();
        if ('Err' in ready.result) throw new Error(ready.result.Err);
        connectedOnce = true;
        setConnection({ state: 'connected' });
        for (;;) {
          const frame = await stream.next();
          if (frame.event !== 'lumen:heartbeat') broadcast(frame);
        }
      } catch (error) {
        stream?.io.destroy();
        if (closed) return;
        setConnection(String(error).includes('different build') ? { state: 'mismatch', detail: String(error) } : { state: connectedOnce ? 'reconnecting' : 'connecting' });
        await sleep(200);
      }
    }
  })();

  const cors = { 'access-control-allow-origin': origin, 'access-control-allow-headers': 'authorization, content-type' };
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'OPTIONS') return res.writeHead(204, cors).end();
    const presented = req.headers.authorization === `Bearer ${token}` || url.searchParams.get('token') === token;
    if (!presented) return res.writeHead(401, cors).end();
    if (url.pathname === '/events') {
      res.writeHead(200, { ...cors, 'content-type': 'text/event-stream', 'cache-control': 'no-store' });
      res.write(': ready\n\n');
      listeners.add(res);
      req.on('close', () => listeners.delete(res));
      return;
    }
    const json = (value) => res.writeHead(200, { ...cors, 'content-type': 'application/json' }).end(JSON.stringify(value));
    if (url.pathname === '/connection') return json(connection);
    // Plugin panels, under the same policy as the shell's lumen-plugin: protocol.
    const panel = url.pathname.match(/^\/panel\/([^/]+)\/([^/]+)$/);
    if (panel && req.method === 'GET') {
      const html = await client.call('plugin_panel', { id: decodeURIComponent(panel[1]), panel: decodeURIComponent(panel[2]) }).catch(() => null);
      const headers = { 'content-security-policy': PANEL_CSP, 'x-content-type-options': 'nosniff', 'content-type': 'text/html; charset=utf-8' };
      return typeof html === 'string' ? res.writeHead(200, headers).end(html) : res.writeHead(404, headers).end('Plugin panel unavailable');
    }
    if (url.pathname === '/call' && req.method === 'POST') {
      let body = '';
      for await (const chunk of req) {
        body += chunk;
        if (body.length > 1 << 20) return res.writeHead(413, cors).end();
      }
      let command, args;
      try {
        ({ command, args } = JSON.parse(body));
      } catch {
        return res.writeHead(400, cors).end();
      }
      if (typeof command !== 'string') return res.writeHead(400, cors).end();
      // Like the shell, never let the page stop or update the service.
      if (command.startsWith('__') && command !== '__status') return json({ error: `Unknown command ${command}` });
      return client.call(command, args).then(
        (ok) => json({ ok }),
        (error) => json({ error: error.message }),
      );
    }
    res.writeHead(404, cors).end();
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  return {
    url: `http://127.0.0.1:${server.address().port}`,
    token,
    /** Plays an event only the shell sends, such as `lumen:update`. */
    emit: (event, payload) => broadcast({ event, payload }),
    async close() {
      closed = true;
      stream?.io.destroy();
      listeners.forEach((res) => res.end());
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
