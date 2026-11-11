// Prepare an isolated release benchmark identity. Never resets existing evidence or accounts.
import { mkdir, readFile, writeFile, copyFile, access } from 'node:fs/promises';
import { constants } from 'node:fs';
import { randomBytes } from 'node:crypto';
import path from 'node:path';
const root = path.resolve(process.env.LUMEN_PERF_ROOT || '.impeccable/performance');
for (const name of ['baseline', 'candidate', 'workspace'])
  await mkdir(path.join(root, name), { recursive: true });
const configPath = path.join(root, 'config.json');
try {
  await access(configPath);
  console.log('Reusing existing isolated performance identity and evidence.');
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
  const identifier = 'io.github.andrea17982.lumen.perf' + randomBytes(4).toString('hex');
  await writeFile(
    configPath,
    JSON.stringify(
      {
        productName: 'Lumen Performance',
        identifier,
        app: {
          windows: [
            {
              title: 'Lumen performance fixture',
              width: 1240,
              height: 820,
              minWidth: 720,
              minHeight: 540,
              visible: false,
              dataDirectory: path.join(root, 'webview'),
            },
          ],
        },
      },
      null,
      2,
    ),
    { flag: 'wx' },
  );
  for (const file of [
    'src/App.tsx',
    'src/components/MessageView.tsx',
    'src/components/MessageBody.tsx',
    'src/components/ExecutionSummary.tsx',
    'src/components/Transcript.tsx',
    'src/lib/useLatestCallback.ts',
    'src/styles.css',
  ])
    await copyFile(file, path.join(root, 'baseline', path.basename(file)), constants.COPYFILE_EXCL);
  await writeFile(
    path.join(root, 'environment.json'),
    JSON.stringify(
      {
        identifier,
        at: new Date().toISOString(),
        platform: process.platform,
        arch: process.arch,
        node: process.version,
      },
      null,
      2,
    ),
    { flag: 'wx' },
  );
  console.log('Prepared isolated identity and source snapshots before candidate edits.');
}
const config = JSON.parse(await readFile(configPath, 'utf8'));
if (!/^io\.github\.andrea17982\.lumen\.perf[a-f0-9]{8}$/.test(config.identifier))
  throw Error('Not a performance-only identifier');
console.log(`Build with: npx tauri build --no-bundle --config "${configPath}"`);
console.log(
  'Freeze target/release/lumen.exe under baseline/ before candidate edits; candidate/ after. No inference is needed.',
);
