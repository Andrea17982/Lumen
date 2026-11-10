// Documentation is part of the release surface: detect stale local links and version drift.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';

const root = process.cwd();
const markdown = new Set(
  execFileSync('git', ['ls-files', '-z', '*.md'], { encoding: 'utf8' })
    .split('\0')
    .filter((file) => file && existsSync(file)),
);
function collect(folder) {
  for (const entry of readdirSync(folder, { withFileTypes: true })) {
    const file = path.join(folder, entry.name);
    if (entry.isDirectory()) collect(file);
    else if (file.endsWith('.md')) markdown.add(file.split(path.sep).join('/'));
  }
}
collect('docs');
const slug = (s) =>
  s
    .replace(/<[^>]*>/g, '')
    .replace(/[*`]/g, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}_\s-]/gu, '')
    .replace(/\s/g, '-');
function headings(file) {
  const result = new Set(),
    counts = new Map();
  for (const match of readFileSync(file, 'utf8').matchAll(/^#{1,6}\s+(.+)$/gm)) {
    const key = slug(match[1]);
    const n = counts.get(key) || 0;
    counts.set(key, n + 1);
    result.add(key + (n ? `-${n}` : ''));
  }
  return result;
}
const problems = [];
let links = 0;
for (const file of markdown) {
  const body = readFileSync(file, 'utf8').replace(/```[^\n]*\n[\s\S]*?```/g, '');
  const matches = [
    ...body.matchAll(/!?\[[^\]\n]*\]\(([^)\n]+)\)/g),
    ...body.matchAll(/(?:src|href)="([^"]+)"/g),
  ];
  for (const match of matches) {
    const href = match[1].replace(/^<|>$/g, '').split(/\s+"/)[0];
    if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('//')) continue;
    const [raw, fragment] = href.split('#');
    const target = path.resolve(path.dirname(file), decodeURIComponent(raw || path.basename(file)));
    links++;
    if (!existsSync(target)) problems.push(`${file}: missing ${href}`);
    else if (
      fragment &&
      target.endsWith('.md') &&
      !headings(target).has(decodeURIComponent(fragment))
    )
      problems.push(`${file}: missing heading ${href}`);
  }
}
const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
const lock = JSON.parse(readFileSync('package-lock.json', 'utf8'));
const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
const cargoLock = readFileSync('src-tauri/Cargo.lock', 'utf8').match(
  /name = "lumen"\r?\nversion = "([^"]+)"/,
)?.[1];
for (const [name, version] of Object.entries({
  lock: lock.version,
  rootPackage: lock.packages[''].version,
  tauri: tauri.version,
  cargo,
  cargoLock,
}))
  if (version !== pkg.version) problems.push(`${name}: ${version} differs from ${pkg.version}`);
if (!existsSync(`docs/release-notes/${pkg.version}.md`))
  problems.push('Current release notes missing');
assert.deepEqual(problems, [], 'Documentation/version consistency failed');
console.log(
  `Documentation passed: ${markdown.size} Markdown files, ${links} local references, version ${pkg.version}.`,
);
