// Native IPC/storage + offline CLI timings. Never attaches the owner's accounts.
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { nativeHarness } from './native-harness.mjs';
import { performanceHarness } from './performance-harness.mjs';

const root = path.resolve(process.env.LUMEN_TURN_LATENCY_ROOT || '.impeccable/turn-latency');
await mkdir(root, { recursive: true });
const executable =
  process.env.LUMEN_LATENCY_EXE || path.resolve('src-tauri/target/debug/lumen.exe');
if (process.env.LUMEN_LATENCY_EXE) {
  process.env.LUMEN_PERF_ROOT = root;
  process.env.PATH = `${path.resolve('tests/fixtures/cli')}${path.delimiter}${process.env.PATH}`;
}
const h = process.env.LUMEN_LATENCY_EXE
  ? await performanceHarness(executable)
  : await nativeHarness('turn-latency', { executable });
// Keep coding-turn checkpoints/context outside the original repository's Git ancestry.
const workspace = process.env.LUMEN_LATENCY_EXE
  ? await mkdtemp(path.join(tmpdir(), 'lumen-latency-workspace-'))
  : h.workspace;
const result = {
  at: new Date().toISOString(),
  build: process.env.LUMEN_LATENCY_EXE ? 'isolated-release' : 'debug',
  sha256: createHash('sha256')
    .update(await readFile(executable))
    .digest('hex'),
  inferenceCalls: 0,
  samples: [],
};
const smoke = process.env.LUMEN_LATENCY_SMOKE === '1';
const flush = () =>
  writeFile(
    path.join(root, smoke ? 'offline-smoke-results.json' : 'offline-results.json'),
    JSON.stringify(result, null, 2),
  );
try {
  const accounts = [];
  for (const provider of ['claude', 'gpt'])
    accounts.push(await h.invoke('add_account', { name: `Offline ${provider}`, provider }));
  for (let repetition = 0; repetition < (smoke ? 1 : 20); repetition++)
    for (const a of repetition % 2 ? [...accounts].reverse() : accounts) {
      const session = await h.invoke('create_session', { workspace });
      let previous;
      for (const mode of ['initial', 'resume']) {
        const start = performance.now();
        await h.invoke('send_message', {
          id: session.id,
          text: 'QUALITY_CONTEXT_FIXTURE',
          options: {
            mode: a.provider,
            accountId: a.id,
            model: a.provider === 'claude' ? 'sonnet' : 'gpt-fixture',
            effort: 'low',
            accessMode: 'autopilot',
            autoReview: false,
            balanceUsage: false,
          },
        });
        const acceptanceMs = performance.now() - start;
        let status;
        const deadline = performance.now() + 30000;
        while (performance.now() < deadline) {
          status = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
            (s) => s.id === session.id,
          )?.status;
          if (['completed', 'error', 'interrupted'].includes(status)) break;
          await new Promise((r) => setTimeout(r, 25));
        }
        const completedMs = performance.now() - start;
        assert.equal(status, 'completed');
        const messages = await h.invoke('get_messages', { id: session.id });
        const out = messages.at(-1);
        const traces = out.activity
          .filter((a) => a.kind === 'execution')
          .map((a) => JSON.parse(a.text));
        assert.equal(traces.length, 1);
        const trace = traces[0];
        assert(trace.endedAt && trace.timings.some(([key]) => key === 'return'));
        assert(trace.timings.some(([key]) => key === 'protocol-ready'));
        assert(trace.timings.some(([key]) => key === 'request-enqueued'));
        assert(!out.activity.some((a) => a.kind === 'latency'));
        const native = JSON.parse(out.activity.findLast((a) => a.kind === 'native').text);
        if (mode === 'resume') assert.equal(native.session, previous);
        previous = native.session;
        const user = messages.findLast((m) => m.role === 'user');
        const prep = JSON.parse(
          user.activity.find((a) => a.kind === 'execution-phase').text,
        ).preparationMs;
        assert(Number.isFinite(prep));
        result.samples.push({
          provider: a.provider,
          repetition,
          mode,
          status,
          acceptanceMs,
          completedMs,
          preparationMs: prep,
          timings: trace.timings,
          normalizedEvents: trace.events,
          firstProcessForAccount: repetition === 0 && mode === 'initial',
        });
        await flush();
        console.log(
          JSON.stringify({
            provider: a.provider,
            repetition,
            mode,
            acceptanceMs: Math.round(acceptanceMs),
            completedMs: Math.round(completedMs),
          }),
        );
      }
    }
  result.completedAt = new Date().toISOString();
  await flush();
} finally {
  await h.close();
}
