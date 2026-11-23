// Same-source size/speed compiler comparison. Alternating order; no accounts or inference.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile, writeFile, copyFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { performanceHarness } from './performance-harness.mjs';
const root = path.resolve('.impeccable/followup-acceptance');
process.env.LUMEN_PERF_ROOT = root;
const rows = [];
for (let pair = -1; pair < 20; pair++)
  for (const arm of pair % 2 ? ['candidate', 'baseline'] : ['baseline', 'candidate']) {
    await new Promise((resolve, reject) => {
      const child = spawn(process.execPath, ['tests/manual/performance-native.mjs', arm], {
        windowsHide: true,
        stdio: 'inherit',
        env: {
          ...process.env,
          LUMEN_PERF_REPETITIONS: '1',
          LUMEN_PERF_WORKLOAD: '5000-mixed',
          LUMEN_PERF_NO_CAPTURES: 'true',
          LUMEN_PERF_SKIP_PRIVATE: pair === 0 ? 'false' : 'true',
        },
      });
      child.on('error', reject);
      child.on('exit', (code) => (code === 0 ? resolve() : reject(Error(`${arm}: ${code}`))));
    });
    const file = path.join(root, `${arm}-results.json`);
    const result = JSON.parse(await readFile(file, 'utf8'));
    assert.equal(result.samples.length, 1);
    if (pair < 0) continue; // Initialize both profiles and the common fixture before timing.
    rows.push({ pair, arm, startupMs: result.startupMs, ...result.samples[0] });
    await copyFile(file, path.join(root, `pair-${pair}-${arm}.json`));
    await writeFile(path.join(root, 'paired-results.json'), JSON.stringify(rows, null, 2));
  }
const cpu = [];
const text = (
  'const example = "calibrazione 東京 🪔";\n' + 'A bounded tokenizer benchmark.\n'
).repeat(3000);
for (const arm of ['baseline', 'candidate']) {
  const executable = path.join(root, arm, 'lumen.exe');
  const h = await performanceHarness(executable);
  try {
    const expected = await h.invoke('count_tokens', { text });
    for (let i = 0; i < 20; i++) {
      const start = performance.now();
      const tokens = await h.invoke('count_tokens', { text });
      assert.equal(tokens, expected);
      cpu.push({
        arm,
        sample: i,
        ms: performance.now() - start,
        tokens,
        utf8Bytes: Buffer.byteLength(text),
      });
    }
  } finally {
    await h.close();
  }
}
await writeFile(path.join(root, 'token-results.json'), JSON.stringify(cpu, null, 2));
const percentile = (values, q) =>
  [...values].sort((a, b) => a - b)[Math.ceil(values.length * q) - 1];
const summary = {
  method:
    '20 alternating AB/BA fresh-process pairs; same source and isolated synthetic 5,000-message mixed history; warm OS/profile. Tokenizer: 20 warm IPC calls/arm.',
  arms: {},
};
for (const arm of ['baseline', 'candidate']) {
  const samples = rows.filter((r) => r.arm === arm);
  const metrics = {};
  for (const key of [
    'startupMs',
    'openMs',
    'typingMs',
    'typingTaskMs',
    'streamTaskMs',
    'heapBytes',
  ])
    metrics[key] = {
      median: percentile(
        samples.map((s) => s[key]),
        0.5,
      ),
      p95: percentile(
        samples.map((s) => s[key]),
        0.95,
      ),
    };
  metrics.tokenMs = {
    median: percentile(
      cpu.filter((r) => r.arm === arm).map((r) => r.ms),
      0.5,
    ),
    p95: percentile(
      cpu.filter((r) => r.arm === arm).map((r) => r.ms),
      0.95,
    ),
  };
  summary.arms[arm] = {
    samples: samples.length,
    exeBytes: (await stat(path.join(root, arm, 'lumen.exe'))).size,
    metrics,
    privateBytes: samples[0].processes?.privateBytes,
  };
}
assert.equal(rows.length, 40);
assert.equal(cpu.length, 40);
await writeFile(path.join(root, 'summary.json'), JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));
