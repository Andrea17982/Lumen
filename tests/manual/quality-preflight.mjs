// Login/catalog checks only; no generation, classification, review or inference.
import path from 'node:path';
import { mkdir, writeFile } from 'node:fs/promises';
import { nativeHarness } from './native-harness.mjs';
import { attachLiveAccounts } from './quality-live-accounts.mjs';

const h = await nativeHarness('quality-preflight', {
  offline: false,
  executable: path.resolve('.impeccable/experiments/quality-baseline/lumen.exe'),
});
let attached;
try {
  attached = await attachLiveAccounts(h);
  const report = {
    at: new Date().toISOString(),
    inferenceCalls: 0,
    accounts: attached.accounts.map(({ provider, ready, reason, model, effort }) => ({
      provider,
      ready,
      reason,
      model: model?.id,
      effort,
    })),
  };
  await mkdir('.impeccable/experiments', { recursive: true });
  await writeFile(
    '.impeccable/experiments/quality-preflight.json',
    JSON.stringify(report, null, 2),
  );
  process.stdout.write(JSON.stringify(report, null, 2) + '\n');
} finally {
  await h.close();
  await attached?.cleanup();
}
