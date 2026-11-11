// Independent task fixtures and oracles. No original project or external asset is copied.
import { cp, mkdir, readFile, writeFile, mkdtemp, symlink, realpath } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:net';
import { chromium } from '@playwright/test';
import { tmpdir } from 'node:os';

export const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const fixtureRoot = path.join(repo, 'tests/fixtures/quality');
export const godotDefault = path.join(
  process.env.LOCALAPPDATA || '',
  'Programs/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe',
);

export function command(executable, args, cwd, timeout = 120000) {
  return new Promise((resolve) => {
    const start = performance.now();
    const child = spawn(executable, args, { cwd, windowsHide: true, stdio: 'pipe' });
    let stdout = '',
      stderr = '',
      expired = false;
    const timer = setTimeout(() => {
      expired = true;
      if (process.platform === 'win32')
        spawn('taskkill', ['/PID', String(child.pid), '/T', '/F'], {
          windowsHide: true,
          stdio: 'ignore',
        });
      else child.kill('SIGKILL');
    }, timeout);
    child.stdout.on('data', (b) => {
      stdout = (stdout + b).slice(-16000);
    });
    child.stderr.on('data', (b) => {
      stderr = (stderr + b).slice(-16000);
    });
    child.on('error', (error) => {
      clearTimeout(timer);
      resolve({ code: -1, stdout, stderr: String(error), elapsedMs: performance.now() - start });
    });
    child.on('close', (code) => {
      clearTimeout(timer);
      resolve({ code: expired ? -2 : code, stdout, stderr, elapsedMs: performance.now() - start });
    });
  });
}
export async function prepareProject(kind, directory, oracle = false) {
  await mkdir(directory, { recursive: true });
  await cp(path.join(fixtureRoot, kind), directory, { recursive: true });
  await writeFile(
    path.join(directory, '.gitignore'),
    '.godot/\nnode_modules/\n.quality-vite-cache/\nquality-check.gd\n*.uid\n',
  );
  if (kind === 'react')
    await symlink(
      path.join(repo, 'node_modules'),
      path.join(directory, 'node_modules'),
      'junction',
    ).catch((e) => {
      if (e.code !== 'EEXIST') throw e;
    });
  if (oracle && kind === 'react') {
    await writeFile(
      path.join(directory, 'counter.ts'),
      'export function advance(value: number, step: number, limit: number): number { return Math.max(0, Math.min(limit, value + step)); }\n',
    );
    const file = path.join(directory, 'CounterView.tsx');
    await writeFile(
      file,
      (await readFile(file, 'utf8')).replace(
        '<button onClick=',
        '<button disabled={value >= limit} onClick=',
      ),
    );
  }
  if (oracle && kind === 'godot') {
    const cs = path.join(directory, 'CounterState.cs');
    await writeFile(
      cs,
      (await readFile(cs, 'utf8')).replace(
        'Value += step;',
        'Value = System.Math.Clamp(Value + step, 0, Limit);',
      ),
    );
    const gd = path.join(directory, 'CounterPanel.gd');
    await writeFile(
      gd,
      (await readFile(gd, 'utf8')) +
        '\t$Panel/Hit.disabled = int(state.get("Value")) >= int(state.get("Limit"))\n',
    );
  }
}

export async function checkProject(kind, directory, godot = godotDefault) {
  // Vite's real module paths do not match Windows 8.3 TEMP aliases.
  directory = await realpath(directory);
  const checks = [];
  if (kind === 'godot') {
    const build = await command('dotnet', ['build', '--nologo'], directory);
    checks.push({ name: 'C# compilation', passed: build.code === 0, ...build });
    if (build.code !== 0) return checks;
    const imported = await command(
      godot,
      ['--headless', '--editor', '--path', directory, '--import', '--quit'],
      directory,
    );
    checks.push({
      name: 'Godot import/GDScript',
      passed:
        imported.code === 0 && !/SCRIPT ERROR|Parse Error/.test(imported.stderr + imported.stdout),
      ...imported,
    });
    await cp(path.join(fixtureRoot, 'check-godot.gd'), path.join(directory, 'quality-check.gd'));
    const runtime = await command(
      godot,
      ['--headless', '--path', directory, '--script', 'res://quality-check.gd'],
      directory,
      30000,
    );
    checks.push({
      name: 'Independent C#/GDScript runtime',
      passed: runtime.code === 0 && runtime.stdout.includes('"passed":true'),
      ...runtime,
    });
    return checks;
  }
  const typecheck = await command(
    process.execPath,
    [
      path.join(repo, 'node_modules/typescript/bin/tsc'),
      '-p',
      path.join(directory, 'tsconfig.json'),
    ],
    directory,
  );
  checks.push({ name: 'TypeScript compilation', passed: typecheck.code === 0, ...typecheck });
  const server = createServer();
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  await new Promise((r) => server.close(r));
  const child = spawn(
    process.execPath,
    [
      path.join(repo, 'node_modules/vite/bin/vite.js'),
      directory,
      '--config',
      path.join(directory, 'vite.config.mjs'),
      '--port',
      String(port),
    ],
    { cwd: repo, windowsHide: true, stdio: 'ignore' },
  );
  let browser;
  try {
    for (let i = 0; i < 100; i++) {
      try {
        await fetch(`http://127.0.0.1:${port}`);
        break;
      } catch {
        await new Promise((r) => setTimeout(r, 100));
      }
    }
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${port}`);
    await page.getByRole('button', { name: 'Hit', exact: true }).waitFor();
    const arithmetic = await page.evaluate(async () => {
      const { advance } = await import('/counter.ts');
      return [advance(2, 100, 3) === 3, advance(1, -100, 3) === 0, advance(0, 20, 0) === 0];
    });
    checks.push({
      name: 'Independent upper/lower/zero bounds',
      passed: arithmetic.every(Boolean),
      values: arithmetic,
    });
    const hit = page.getByRole('button', { name: 'Hit', exact: true });
    const output = page.getByLabel('Value', { exact: true });
    const initial = (await output.textContent())?.trim() === '0 / 3';
    for (let i = 0; i < 3; i++) await hit.click();
    const maximum = (await output.textContent())?.trim() === '3 / 3';
    const disabled = await hit.isDisabled();
    await page.getByRole('button', { name: 'Reset', exact: true }).click();
    const reset = (await output.textContent())?.trim() === '0 / 3' && (await hit.isEnabled());
    checks.push({
      name: 'Independent React UI state/label/disable/reset',
      passed: initial && maximum && disabled && reset,
      initial,
      maximum,
      disabled,
      reset,
    });
  } catch (error) {
    checks.push({ name: 'Independent React runtime', passed: false, error: String(error) });
  } finally {
    await browser?.close();
    if (child.exitCode === null) child.kill();
  }
  return checks;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = path.join(repo, '.impeccable/experiments/quality-projects');
  await mkdir(root, { recursive: true });
  const work = await mkdtemp(path.join(tmpdir(), 'lumen-quality-projects-'));
  const results = [];
  for (const kind of ['react', 'godot']) {
    for (const oracle of [false, true]) {
      const directory = path.join(work, `${kind}-${oracle ? 'oracle' : 'baseline'}`);
      await prepareProject(kind, directory, oracle);
      const checks = await checkProject(kind, directory);
      results.push({ kind, oracle, directory, checks });
      process.stdout.write(
        JSON.stringify({
          kind,
          oracle,
          checks: checks.map(({ name, passed, code }) => ({ name, passed, code })),
        }) + '\n',
      );
    }
  }
  await writeFile(
    path.join(root, 'local-results.json'),
    JSON.stringify({ at: new Date().toISOString(), results }, null, 2),
  );
  const valid = ['react', 'godot'].every(
    (kind) =>
      results.find((r) => r.kind === kind && r.oracle)?.checks.every((c) => c.passed) &&
      results.find((r) => r.kind === kind && !r.oracle)?.checks.some((c) => !c.passed),
  );
  if (!valid) process.exitCode = 1;
}
