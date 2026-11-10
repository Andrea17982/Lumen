import { copyFile, cp, mkdir, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const source = path.join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis');
const portable = path.join(root, 'src-tauri', 'target', 'release');
const destination = path.join(root, 'builds', 'latest');
const files = (await readdir(source)).filter((name) => name.toLowerCase().endsWith('.exe'));
if (files.length === 0) throw new Error(`No NSIS installer found in ${source}`);
await mkdir(destination, { recursive: true });
const newest = await files.reduce(async (bestPromise, file) => {
  const best = await bestPromise;
  return (await stat(path.join(source, file))).mtimeMs >
    (await stat(path.join(source, best))).mtimeMs
    ? file
    : best;
}, Promise.resolve(files[0]));
const portableName = newest.replace(/-setup\.exe$/i, '-portable.exe');
if (portableName === newest)
  throw new Error(`Installer name does not identify its portable counterpart: ${newest}`);
for (const file of await readdir(destination)) {
  if (!file.toLowerCase().endsWith('.exe')) continue;
  try {
    await rm(path.join(destination, file), { force: true });
  } catch (error) {
    if (!['EPERM', 'EBUSY', 'EACCES'].includes(error?.code)) throw error;
    console.warn(`Keeping in-use executable ${file}`);
  }
}
// Refresh the portable build from the just-built application; never copy a stale target artifact.
await copyFile(path.join(portable, 'lumen.exe'), path.join(portable, portableName));
await copyFile(path.join(source, newest), path.join(destination, newest));
const portableDestination = path.join(destination, portableName);
let copiedPortableName = portableName;
try {
  await rm(portableDestination, { force: true });
  await copyFile(path.join(portable, portableName), portableDestination);
} catch (error) {
  if (!['EPERM', 'EBUSY', 'EACCES'].includes(error?.code)) throw error;
  const date = new Date()
    .toISOString()
    .replace(/[-:]/g, '')
    .replace(/\.\d{3}Z$/, 'Z');
  const updatedName = portableName.replace(/-portable\.exe$/i, `-portable-updated-${date}.exe`);
  await copyFile(path.join(portable, portableName), path.join(destination, updatedName));
  copiedPortableName = updatedName;
  console.warn(`Portable executable is in use; updated copy saved as ${updatedName}`);
}
// A portable executable needs its pinned browser and notices alongside it. The ZIP is
// the distributable; the loose copy is convenient when running directly from builds/latest.
const moli = path.join(root, 'src-tauri/resources/moli', `${process.platform}-${process.arch}`);
await cp(moli, path.join(destination, 'resources/moli', `${process.platform}-${process.arch}`), { recursive: true });
const zipName = copiedPortableName.replace(/\.exe$/i, '.zip');
execFileSync('powershell.exe', ['-NoProfile', '-File', path.join(root, 'scripts/package-portable.ps1'),
  '-Executable', path.join(portable, 'lumen.exe'), '-Moli', moli,
  '-Destination', path.join(destination, zipName)], { windowsHide: true, stdio: 'pipe' });
// The manifest belongs to this build, including the renamed copy when the old EXE is running.
const checksums = await Promise.all(
  [newest, copiedPortableName, zipName].map(async (name) => {
    const hash = createHash('sha256')
      .update(await readFile(path.join(destination, name)))
      .digest('hex');
    return `${hash}  ${name}`;
  }),
);
await writeFile(path.join(destination, 'SHA256SUMS.txt'), checksums.join('\n') + '\n');
console.log(`Installer and portable build copied to ${path.relative(root, destination)}`);
