// Common semantic acceptance corpus. Protocol-specific fixtures retain their native wire formats.
import { spawn } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
const suites = [
  'native-smoke',
  'native-shared-integrations',
  'native-connectors',
  'native-resume-replay',
  'native-quota-replay',
  'native-081-continuity',
  'native-routing',
  'native-antigravity',
  'native-opencode',
  'native-api',
  'native-cursor',
  'native-menu-layout',
];
const results = [];
for (const suite of suites) {
  console.log(`START ${suite}`);
  const start = Date.now();
  const child = spawn(process.execPath, [`tests/${suite}.mjs`], {
    windowsHide: true,
    stdio: 'pipe',
  });
  let output = '';
  for (const stream of [child.stdout, child.stderr])
    stream.on('data', (b) => {
      output += b;
      process.stdout.write(b);
    });
  const code = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', resolve);
  });
  results.push({ suite, exitCode: code, durationMs: Date.now() - start, output });
  await writeFile(
    '.codex/verification/09-adapter-corpus.json',
    JSON.stringify(
      {
        passed: results.length === suites.length && results.every((x) => x.exitCode === 0),
        scope:
          'Offline Windows WebView2/IPC/storage/keychain/processes and actual Lumen tools. Four fundamental agents plus three official API wire adapters; protocol-specific scenarios. No paid inference.',
        results,
      },
      null,
      2,
    ),
  );
  if (code !== 0) process.exit(code || 1);
  console.log(`PASS ${suite}`);
}
