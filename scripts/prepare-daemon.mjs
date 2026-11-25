import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8', windowsHide: true })
  .match(/^host: (.+)$/m)[1]
  .trim();
const target = process.env.TAURI_ENV_TARGET_TRIPLE || host;
const debug = process.argv.includes('--debug') || process.env.TAURI_ENV_DEBUG === 'true';
const profile = debug ? 'debug' : 'release';
const extension = target.includes('windows') ? '.exe' : '';
const targets =
  target === 'universal-apple-darwin' ? ['aarch64-apple-darwin', 'x86_64-apple-darwin'] : [target];
const binaries = [];
mkdirSync(path.join(root, 'src-tauri/binaries'), { recursive: true });
for (const arch of targets) {
  const args = [
    'build',
    '--manifest-path',
    'src-tauri/Cargo.toml',
    '-p',
    'lumen-daemon',
    '--locked',
  ];
  if (!debug) args.push('--release');
  if (arch !== host) args.push('--target', arch);
  execFileSync('cargo', args, { cwd: root, stdio: 'inherit', windowsHide: true });
  binaries.push(
    path.join(
      root,
      'src-tauri/target',
      arch === host ? '' : arch,
      profile,
      `lumen-daemon${extension}`,
    ),
  );
  copyFileSync(
    binaries.at(-1),
    path.join(root, 'src-tauri/binaries', `lumen-daemon-${arch}${extension}`),
  );
}
const output = path.join(root, 'src-tauri/binaries', `lumen-daemon-${target}${extension}`);
if (binaries.length === 2)
  execFileSync('lipo', ['-create', ...binaries, '-output', output], { stdio: 'inherit' });
console.log(`Prepared ${profile} service: ${output}`);
