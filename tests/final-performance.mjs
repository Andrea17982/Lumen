// Serial Windows release regression check, never infers or uses the real profile.
import { spawn } from 'node:child_process';
import { copyFile, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
const root = '.impeccable/final-acceptance';
for (const [label, arm] of [
  ['baseline-a', 'baseline'],
  ['candidate-a', 'candidate'],
  ['baseline-b', 'baseline'],
  ['candidate-b', 'candidate'],
]) {
  let block;
  for (let launch = 0; launch < 20; launch++) {
    console.log(`Fresh process ${label} ${launch + 1}/20`);
    await new Promise((resolve, reject) => {
      const child = spawn(process.execPath, ['tests/performance-native.mjs', arm], {
        windowsHide: true,
        stdio: 'inherit',
        env: {
          ...process.env,
          LUMEN_PERF_ROOT: root,
          LUMEN_PERF_REPETITIONS: '1',
          LUMEN_PERF_WORKLOAD: '5000-mixed',
          LUMEN_PERF_NO_CAPTURES: launch === 19 ? 'false' : 'true',
          LUMEN_PERF_SKIP_PRIVATE: launch === 0 || launch === 19 ? 'false' : 'true',
        },
      });
      child.on('error', reject);
      child.on('exit', (code) =>
        code === 0 ? resolve() : reject(new Error(`${label} exited ${code}`)),
      );
    });
    const source = path.join(root, `${arm}-results.json`);
    await copyFile(source, path.join(root, `fresh-${label}-${launch}.json`));
    const record = JSON.parse(await readFile(source, 'utf8'));
    block ||= {
      ...record,
      samples: [],
      launches: [],
      method: 'One workload sample per fresh process; same short-chat warmup in every launch',
    };
    block.samples.push(...record.samples.map((sample) => ({ ...sample, repetition: launch })));
    block.launches.push({
      startupMs: record.startupMs,
      windowState: record.windowState,
      sha256: record.sha256,
    });
    block.completedAt = record.completedAt;
    await writeFile(path.join(root, `${label}.json`), JSON.stringify(block, null, 2));
    await new Promise((resolve) => setTimeout(resolve, 2000));
  }
}
