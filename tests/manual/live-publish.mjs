// Explicit opt-in: a real Publish and correction loop on a disposable GitHub repository.
// A large Auto task runs in its own worktree, Lumen publishes the pull request, a CI-only
// check fails, the correction loop fixes it and the pull request is merged. Uses the Claude
// Code account already in Lumen (subscription) through a temporary junction, and the gh CLI
// already signed in on this machine. Never point it at a repository you care about.
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, readdir, rmdir, symlink, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
if (process.env.LUMEN_LIVE_PUBLISH !== '1')
  throw Error('Set LUMEN_LIVE_PUBLISH=1 to authorize a real pull request on GitHub.');
const repo = process.env.LUMEN_PUBLISH_REPO;
assert(repo && !/lumen$/i.test(repo), 'Set LUMEN_PUBLISH_REPO to a disposable owner/name');
const minutes = Number(process.env.LUMEN_PUBLISH_MINUTES || 40);
const original =
  process.env.LUMEN_LIVE_ACCOUNT_ROOT ||
  path.join(process.env.APPDATA, 'io.github.andrea17982.lumen');
const claude = JSON.parse(
  execFileSync(
    'python',
    [
      '-c',
      'import sqlite3,sys,json; c=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True); print(json.dumps(c.execute("SELECT id FROM accounts WHERE provider=\'claude\'").fetchall()))',
      path.join(original, 'lumen.db'),
    ],
    { encoding: 'utf8', windowsHide: true },
  ),
)[0]?.[0];
assert(claude, 'Lumen has no Claude Code account');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

const root = process.cwd();
const data = await mkdtemp(path.join(tmpdir(), 'lumen-live-publish-'));
const workspace = path.join(data, 'project');
execFileSync('gh', ['repo', 'clone', repo, workspace], { stdio: 'ignore', windowsHide: true });
await writeFile(
  path.join(data, 'settings.json'),
  JSON.stringify({ orchestra: false, closeToTray: false }),
);
const exe = process.env.LUMEN_APP_EXE || path.join(root, 'src-tauri/target/debug/lumen.exe');
const child = spawn(exe, [], {
  cwd: root,
  windowsHide: true,
  stdio: 'ignore',
  env: {
    ...process.env,
    LUMEN_TEST_DATA_DIR: data,
    WEBVIEW2_USER_DATA_FOLDER: path.join(data, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9227',
  },
});
let browser;
let link;
try {
  for (let i = 0; i < 100 && !browser; i++) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9227');
    } catch {
      await sleep(200);
    }
  }
  assert(browser, 'The isolated native app did not expose its test endpoint');
  const page = browser
    .contexts()[0]
    .pages()
    .find((p) => !p.url().startsWith('devtools:'));
  await page.locator('h1').first().waitFor();
  const invoke = (command, args = {}) =>
    page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
      command,
      args,
    });
  assert.equal((await invoke('diagnostics')).dataDir, data);
  const a = await invoke('add_account', { name: 'Live Claude Publish', provider: 'claude' });
  const isolated = path.join(data, 'accounts', a.id);
  for (const name of await readdir(isolated)) await unlink(path.join(isolated, name));
  await rmdir(isolated);
  await symlink(path.join(original, 'accounts', claude), isolated, 'junction');
  link = isolated;
  assert.equal(await invoke('account_status', { id: a.id }), true, 'Claude is not signed in');

  const s = await invoke('create_session', { workspace });
  const status = async () =>
    (await invoke('list_sessions', { search: '', offset: 0 })).find((x) => x.id === s.id)?.status;
  const idle = async (limit) => {
    for (let i = 0; i < limit * 2; i++) {
      const st = await status();
      if (['completed', 'error', 'interrupted'].includes(st)) return st;
      await sleep(500);
    }
    await invoke('cancel_run', { id: s.id }).catch(() => {});
    return 'timeout';
  };
  const kinds = async () =>
    (await invoke('get_messages', { id: s.id })).flatMap((m) =>
      m.activity.map((x) => x.kind + ': ' + x.text.slice(0, 160).replace(/\s+/g, ' ')),
    );
  await invoke('send_message', {
    id: s.id,
    // LUMEN_PUBLISH_PROMPT replaces the task, e.g. once the default one is merged.
    text:
      process.env.LUMEN_PUBLISH_PROMPT ||
      'Turn this project into a small calculator package across several modules. ' +
        '1) src/math.js: add multiply, divide (RangeError on division by zero), power (integer ' +
        'exponents, negative ones too) and clamp. 2) New src/stats.js: mean, median, mode and ' +
        'stddev over arrays, with TypeError on empty or non-numeric input. 3) New src/parse.js: ' +
        'parseExpression("2 + 3 * 4") evaluating + - * / with precedence and parentheses, no eval. ' +
        '4) New bin/calc.js CLI: `node bin/calc.js "<expr>"` prints the result, ' +
        '`node bin/calc.js stats 1 2 3` prints all four statistics, exit code 1 with a message on ' +
        'bad input; add a "bin" entry to package.json. 5) Tests for every function and the CLI ' +
        '(spawn it), including error cases, one test file per module. Keep the existing style.',
    options: { mode: 'auto', effort: '', accessMode: 'autopilot', autoReview: false },
  });
  const first = await idle(900);
  log('first turn', first);
  const activity = await kinds();
  for (const k of activity.filter((x) => /^(task-|validation|route|error)/.test(x))) log(' ', k);
  const st = await invoke('task_worktree_status', { id: s.id, preview: true });
  assert(st, 'The task did not run in a worktree');
  log('worktree', st.branch, 'ahead', st.ahead, 'dirty', st.dirty, 'blocked', st.blocked);
  assert(st.ahead > 0, 'Lumen did not commit the task');
  const subject = execFileSync('git', ['-C', st.path, 'log', '-1', '--format=%s'], {
    encoding: 'utf8',
  }).trim();
  const published = await invoke('publish_task', {
    id: s.id,
    base: st.base,
    title: subject,
    body: 'Live Publish test from Lumen.',
  });
  log('published', published.url, 'auto-merge error:', published.autoMergeError);

  // The correction loop ticks every minute while the conversation is idle.
  let last = '';
  let state = '';
  const deadline = Date.now() + minutes * 60_000;
  while (Date.now() < deadline) {
    await sleep(15_000);
    if ((await status()) === 'running') {
      if (last !== 'running') log('correction turn running');
      last = 'running';
      continue;
    }
    const now = await invoke('task_worktree_status', { id: s.id, preview: false });
    const pr = now?.pr;
    state = JSON.parse(
      execFileSync('gh', ['pr', 'view', published.url, '--json', 'state'], { encoding: 'utf8' }),
    ).state;
    const line = (pr ? pr.last + ' · rounds ' + pr.rounds : 'no task') + ' · GitHub ' + state;
    if (line !== last) log(line);
    last = line;
    if (state === 'MERGED' && !now) break;
    if (pr && !pr.watching) break;
  }
  log('final', state, last);
  for (const k of (await kinds()).filter((x) => /^(task-|error)/.test(x))) log(' ', k);
  const notes = (await invoke('get_messages', { id: s.id }))
    .filter((m) => m.role === 'assistant' && m.text.startsWith('Lumen ·'))
    .map((m) => m.text.slice(0, 300));
  for (const n of notes) log('note:', n);
  assert.equal(state, 'MERGED', 'The pull request was not merged');
} finally {
  if (link) await unlink(link).catch(() => rmdir(link).catch(() => {}));
  await browser?.close().catch(() => {});
  child.kill();
}
