import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// Local releases target the build machine's instruction set; CI distributables stay portable.
const root = fileURLToPath(new URL('../', import.meta.url));
execFileSync(process.execPath, ['node_modules/@tauri-apps/cli/tauri.js', 'build'], {
  cwd: root,
  stdio: 'inherit',
  windowsHide: true,
  env: { ...process.env, RUSTFLAGS: `${process.env.RUSTFLAGS ?? ''} -C target-cpu=native`.trim() },
});
execFileSync(process.execPath, ['scripts/copy-release.mjs'], {
  cwd: root,
  stdio: 'inherit',
  windowsHide: true,
});
