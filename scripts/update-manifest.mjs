// Writes the updater manifest (latest.json) for a release from its signed artifacts.
// Usage: node scripts/update-manifest.mjs <version> <artifact folder> [notes]
// The folder holds the Windows setup and the macOS app archive, each with its .sig from
// `tauri build` (signing key in TAURI_SIGNING_PRIVATE_KEY). A platform without a signature is
// left out: that build updates from the release page.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const [version, folder, notes = ''] = process.argv.slice(2);
if (!/^\d+\.\d+\.\d+$/.test(version ?? '') || !folder) {
  console.error('Usage: node scripts/update-manifest.mjs <version> <artifact folder> [notes]');
  process.exit(2);
}
const base = `https://github.com/Andrea17982/Lumen/releases/download/v${version}`;
const files = readdirSync(folder, { recursive: true }).map(String);
const find = (suffix) => files.find((f) => path.basename(f) === `Lumen_${version}_${suffix}`);
const platform = (suffix) => {
  const file = find(suffix);
  const sig = find(`${suffix}.sig`);
  if (!file || !sig) return null;
  return { signature: readFileSync(path.join(folder, sig), 'utf8').trim(), url: `${base}/${path.basename(file)}` };
};
const platforms = {};
const windows = platform('x64-setup.exe');
if (windows) platforms['windows-x86_64'] = windows;
const mac = platform('macos-universal.app.tar.gz');
if (mac) platforms['darwin-aarch64'] = platforms['darwin-x86_64'] = mac;
if (!windows) {
  console.error('The Windows setup or its signature is missing: no manifest written.');
  process.exit(1);
}
const manifest = { version, notes, pub_date: new Date().toISOString(), platforms };
writeFileSync(path.join(folder, 'latest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`latest.json: ${Object.keys(platforms).join(', ')}`);
