import { copyFile, mkdir, readdir, rm, stat } from 'node:fs/promises';
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
  console.warn(`Portable executable is in use; updated copy saved as ${updatedName}`);
}
console.log(`Installer and portable build copied to ${path.relative(root, destination)}`);
