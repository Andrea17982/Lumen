// Explicit opt-in: tiny real subscription-CLI turns against disposable project folders.
// Authentication remains in each official CLI's existing account directory; no token is read
// or copied by this harness. Temporary junctions are unlinked, never recursively traversed.
import { spawn, execFileSync } from 'node:child_process';
import {
  mkdtemp,
  mkdir,
  readdir,
  rmdir,
  symlink,
  unlink,
  readFile,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { createServer } from 'node:http';
if (process.env.LUMEN_LIVE_NATIVE !== '1')
  throw Error('Set LUMEN_LIVE_NATIVE=1 to authorize the live CLI probe.');
const original =
  process.env.LUMEN_LIVE_ACCOUNT_ROOT ||
  path.join(process.env.APPDATA, 'io.github.andrea17982.lumen');
const metadata = execFileSync(
  'python',
  [
    '-c',
    'import sqlite3,sys,json; c=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True); print(json.dumps(c.execute("SELECT provider,id FROM accounts").fetchall()))',
    path.join(original, 'lumen.db'),
  ],
  { encoding: 'utf8', windowsHide: true },
);
const accounts = JSON.parse(metadata).map(([provider, id]) => ({ provider, id }));
const root = process.cwd();
const data = await mkdtemp(path.join(tmpdir(), 'lumen-live-native-'));
const links = [];
await writeFile(
  path.join(data, 'settings.json'),
  JSON.stringify({
    repoContext: false,
    verifyChanges: false,
    orchestra: false,
    closeToTray: false,
  }),
);
const child = spawn(path.join(root, 'src-tauri/target/debug/lumen.exe'), [], {
  cwd: root,
  windowsHide: true,
  stdio: 'ignore',
  env: {
    ...process.env,
    LUMEN_TEST_DATA_DIR: data,
    WEBVIEW2_USER_DATA_FOLDER: path.join(data, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9225',
  },
});
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
let browser;
let invoke;
const results = [];
const moliFixture = createServer((_request, response) => {
  response.setHeader('Content-Type', 'text/html; charset=utf-8');
  response.end(
    '<html><head><title>Moli native acceptance</title></head><body><h1>MOLI_NATIVE_RESULT_9173</h1><p id="out">before</p><button id="go" onclick="document.getElementById(\'out\').textContent=\'MOLI_CLICKED\'">Apply</button></body></html>',
  );
});
await new Promise((resolve) => moliFixture.listen(0, '127.0.0.1', resolve));
const moliAddress = `http://127.0.0.1:${moliFixture.address().port}/`;
try {
  for (let i = 0; i < 100; i++) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9225');
      break;
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
  invoke = (command, args = {}) =>
    page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
      command,
      args,
    });
  assert.equal((await invoke('diagnostics')).dataDir, data);
  for (const provider of (process.env.LUMEN_LIVE_PROVIDERS || 'claude,gpt').split(',')) {
    const actual = accounts.find((a) => a.provider === provider);
    assert(actual, `${provider} has no configured Lumen account`);
    const a = await invoke('add_account', { name: `Live ${provider} probe`, provider });
    const isolated = path.join(data, 'accounts', a.id);
    const generated = await readdir(isolated);
    assert(generated.every((name) => name === 'config.toml'));
    for (const name of generated) await unlink(path.join(isolated, name));
    await rmdir(isolated);
    await symlink(path.join(original, 'accounts', actual.id), isolated, 'junction');
    links.push(isolated);
    assert.equal(
      await invoke('account_status', { id: a.id }),
      true,
      `${provider} is not signed in`,
    );
    const models = await invoke('list_models', { id: a.id, refresh: true });
    const model =
      (provider === 'claude'
        ? models.find((m) => /haiku/i.test(m.id + ' ' + m.name))
        : models.find((m) => m.isDefault)) || models[0];
    assert(model, 'No selectable native model');
    const workspace = path.join(data, `project-${provider}`);
    await mkdir(workspace);
    const s = await invoke('create_session', { workspace });
    const started = Date.now();
    const options = {
      mode: provider,
      accountId: a.id,
      model: model.id,
      effort: model.efforts.includes('low') ? 'low' : model.defaultEffort || '',
      accessMode: 'autopilot',
    };
    if (process.env.LUMEN_LIVE_MOLI === '1') {
      await invoke('send_message', {
        id: s.id,
        options,
        text: `Bounded browser acceptance: use lumen_browser (Moli) browser_navigate to open ${moliAddress}, then browser_click with selector #go, then browser_extract ${process.env.LUMEN_LIVE_MOLI_NO_IMAGE === '1' ? '' : 'and browser_screenshot'}. Reply with the exact final #out text. Do not use another browser, native web tools, shell network commands or subagents. Do not ask permission: this is already authorized in Autopilot.`,
      });
      let first;
      for (let i = 0; i < 900; i++) {
        const status = (await invoke('list_sessions', { search: '', offset: 0 })).find(
          (row) => row.id === s.id,
        )?.status;
        const rows = await invoke('get_messages', { id: s.id });
        assert.equal((await invoke('pending_questions')).length, 0, 'Autopilot issued a question');
        if (status === 'error') throw Error(JSON.stringify(rows.at(-1)));
        if (status === 'completed') {
          first = rows.at(-1);
          break;
        }
        await sleep(200);
      }
      assert(first?.text.includes('MOLI_CLICKED'), JSON.stringify(first));
      assert(first.activity.some((a) => a.kind === 'browser' && a.text.includes('browser_click')));
      const image =
        process.env.LUMEN_LIVE_MOLI_NO_IMAGE !== '1' &&
        JSON.parse(first.activity.find((a) => a.kind === 'image').text);
      if (image)
        assert(
          (await readFile(path.join(image.folder, image.path)))
            .subarray(0, 4)
            .equals(Buffer.from([137, 80, 78, 71])),
        );
      if (!['claude', 'gpt'].includes(provider)) {
        results.push({
          provider,
          model: model.id,
          moli: true,
          image: Boolean(image),
          passed: true,
          elapsedMs: Date.now() - started,
        });
        console.log(JSON.stringify(results.at(-1)));
        continue;
      }
      const native = JSON.parse(first.activity.find((a) => a.kind === 'native').text);
      await invoke('send_message', {
        id: s.id,
        options,
        text: 'Use Moli browser_extract on the existing tab, without navigating again. Reply only with the #out text.',
      });
      let follow;
      for (let i = 0; i < 600; i++) {
        const status = (await invoke('list_sessions', { search: '', offset: 0 })).find(
          (row) => row.id === s.id,
        )?.status;
        if (status === 'error')
          throw Error(JSON.stringify((await invoke('get_messages', { id: s.id })).at(-1)));
        if (status === 'completed') {
          follow = (await invoke('get_messages', { id: s.id })).at(-1);
          break;
        }
        await sleep(200);
      }
      assert(follow?.text.includes('MOLI_CLICKED'), JSON.stringify(follow));
      assert.equal(
        JSON.parse(follow.activity.find((a) => a.kind === 'native').text).session,
        native.session,
      );
      results.push({
        provider,
        model: model.id,
        moli: true,
        image: true,
        resumed: true,
        passed: true,
        elapsedMs: Date.now() - started,
      });
      console.log(JSON.stringify(results.at(-1)));
      continue;
    }
    const tool = provider === 'claude' ? 'AskUserQuestion' : 'lumen_request_user_input';
    await invoke('send_message', {
      id: s.id,
      text: `This is a bounded Lumen protocol test in this disposable folder. Before writing anything, call ${tool} with exactly one question asking whether to create probe.txt, and options Continue and Cancel. I have not selected an option yet, so wait for the structured answer. After Continue, write probe.txt containing exactly NATIVE_OK, then reply NATIVE_OK. Use the native question tool, not a plain-text question or plan. Do not read any unrelated files, run subagents, access the network or call other external services.`,
      options,
    });
    let answered = false;
    let first;
    for (let i = 0; i < 900; i++) {
      const messages = await invoke('get_messages', { id: s.id });
      const pending = await invoke('pending_questions');
      const event = messages.flatMap((m) => m.activity).find((a) => a.kind === 'interaction');
      if (event && !answered) {
        const request = JSON.parse(event.text);
        if (pending.includes(request.id)) {
          await invoke('answer_question', {
            id: request.id,
            answers: Object.fromEntries(request.questions.map((q) => [q.id, ['Continue']])),
          });
          answered = true;
        }
      }
      const status = (await invoke('list_sessions', { search: '', offset: 0 })).find(
        (row) => row.id === s.id,
      )?.status;
      if (status === 'error')
        throw Error(
          `${provider}: ${messages
            .at(-1)
            ?.activity.filter((a) => a.kind === 'error')
            .map((a) => a.text)
            .join('; ')}`,
        );
      if (status === 'completed') {
        first = messages.at(-1);
        break;
      }
      await sleep(200);
    }
    assert(answered, `${provider} did not issue a structured native question`);
    assert(first, `${provider} did not finish`);
    assert.equal((await readFile(path.join(workspace, 'probe.txt'), 'utf8')).trim(), 'NATIVE_OK');
    const native = JSON.parse(first.activity.find((a) => a.kind === 'native').text);
    await invoke('send_message', {
      id: s.id,
      text: 'Read the probe.txt file you just wrote. Reply with only its contents and do not change files.',
      options,
    });
    let follow;
    for (let i = 0; i < 600; i++) {
      const rows = await invoke('list_sessions', { search: '', offset: 0 });
      const status = rows.find((row) => row.id === s.id)?.status;
      if (status === 'completed') {
        follow = (await invoke('get_messages', { id: s.id })).at(-1);
        break;
      }
      if (status === 'error') throw Error(`${provider} resume failed`);
      await sleep(200);
    }
    assert(follow?.text.includes('NATIVE_OK'));
    assert(follow.activity.some((a) => a.kind === 'resume'));
    assert.equal(
      JSON.parse(follow.activity.find((a) => a.kind === 'native').text).session,
      native.session,
    );
    results.push({
      provider,
      model: model.id,
      passed: true,
      resumed: true,
      elapsedMs: Date.now() - started,
    });
    console.log(JSON.stringify(results.at(-1)));
    if (provider === 'gpt' && process.env.LUMEN_LIVE_GAMEPLAY_ROOT) {
      const project = await import('node:fs/promises').then((fs) =>
        fs.realpath(process.env.LUMEN_LIVE_GAMEPLAY_ROOT),
      );
      assert(
        project
          .toLowerCase()
          .startsWith(path.join(root, '.codex', 'acceptance').toLowerCase() + path.sep),
      );
      const godot = process.env.LUMEN_GODOT_EXE;
      assert(godot);
      const session = await invoke('create_session', { workspace: project });
      const graphify = process.env.LUMEN_GRAPHIFY_EXE;
      assert(graphify, 'Set LUMEN_GRAPHIFY_EXE for gameplay acceptance');
      await invoke('send_message', {
        id: session.id,
        text: `Implement the already authorized bounded offline gameplay acceptance probe in this isolated Rodent Heroes copy. Only create game/lumen_acceptance/training.gd, training.tscn and check.gd. Use ordinary built-in Godot controls, no art assets. The gameplay contract is: a training target starts at 3 HP, Hit reduces HP by 1 clamped at 0, Hit becomes disabled at 0, Reset restores 3 HP and enables Hit, and a label displays current/max HP. The standalone scene uses training.gd and never starts the actual client, contacts services or alters game balance. check.gd must run headlessly as SceneTree, load that scene, exercise the buttons' pressed signals, verify state, label, disabled-at-zero and reset behavior, and exit nonzero on failure. Follow CLAUDE.md. Graphify has already extracted the code-only graph; before code use '${graphify}' explain and affected on the existing project node game_rodentheroes_csproj_game_rodentheroes. This probe has no dependency on existing game code and no production UI/art changes, so no new product/visual approval is needed. Apply the code now, then actually run '${godot}' --headless --path game --script res://lumen_acceptance/check.gd and fix any error. Verify git diff --check. Do not edit any existing file, including project.godot, do not export/deploy/use paid tools or network, and do not start subagents. Reply with paths and the actual runtime-check result.`,
        options,
      });
      let done = false;
      for (let i = 0; i < 1800; i++) {
        const status = (await invoke('list_sessions', { search: '', offset: 0 })).find(
          (s) => s.id === session.id,
        )?.status;
        if (status === 'error')
          throw Error(
            'Gameplay acceptance failed: ' +
              JSON.stringify((await invoke('get_messages', { id: session.id })).at(-1)),
          );
        if (status === 'completed') {
          done = true;
          break;
        }
        await sleep(200);
      }
      assert(done, 'Gameplay acceptance timed out');
      const output = execFileSync(
        godot,
        [
          '--headless',
          '--path',
          path.join(project, 'game'),
          '--script',
          'res://lumen_acceptance/check.gd',
        ],
        { encoding: 'utf8', windowsHide: true, timeout: 30000 },
      );
      assert(
        (await readFile(path.join(project, 'game/lumen_acceptance/training.gd'), 'utf8')).length >
          0,
      );
      results.push({
        project: 'Rodent Heroes isolated offline gameplay',
        passed: true,
        scene: 'res://lumen_acceptance/training.tscn',
        runtimeCheck: output.slice(-2000),
      });
      console.log(JSON.stringify(results.at(-1)));
    }
    if (provider === 'gpt' && process.env.LUMEN_LIVE_GRAPH === '1') {
      const graphRoot = path.join(data, 'graph-project');
      await mkdir(path.join(graphRoot, 'src'), { recursive: true });
      await writeFile(path.join(graphRoot, 'src/lib.rs'), 'pub fn answer() -> u32 { 42 }\n');
      const prefs = await invoke('get_settings');
      await invoke('save_settings', { value: { ...prefs, repoContext: true } });
      const s = await invoke('create_session', { workspace: graphRoot });
      await invoke('send_message', {
        id: s.id,
        text: 'Use the configured lumen_graph MCP find_symbol tool to find answer. Report its file and line from the tool result. Do not use shell commands, change files, start subagents, search the internet or access other services. This is a read-only graph integration probe.',
        options,
      });
      let answer;
      for (let i = 0; i < 900; i++) {
        const status = (await invoke('list_sessions', { search: '', offset: 0 })).find(
          (row) => row.id === s.id,
        )?.status;
        if (status === 'error')
          throw Error(
            'Native graph probe failed: ' +
              JSON.stringify((await invoke('get_messages', { id: s.id })).at(-1)),
          );
        if (status === 'completed') {
          answer = (await invoke('get_messages', { id: s.id })).at(-1);
          break;
        }
        await sleep(200);
      }
      assert(answer?.text.includes('src/lib.rs'));
      assert(
        answer.activity.some((a) => a.kind === 'tool' && /find_symbol/.test(a.text)),
        JSON.stringify(answer.activity),
      );
      results.push({ provider, graph: true, passed: true });
      console.log(JSON.stringify(results.at(-1)));
      await invoke('save_settings', { value: prefs });
    }
    if (provider === 'gpt' && process.env.LUMEN_LIVE_ACCEPTANCE_ROOT) {
      const acceptance = await import('node:fs/promises').then((fs) =>
        fs.realpath(process.env.LUMEN_LIVE_ACCEPTANCE_ROOT),
      );
      assert(
        acceptance
          .toLowerCase()
          .startsWith(path.join(root, '.codex', 'acceptance').toLowerCase() + path.sep),
        'Live project acceptance must use the isolated acceptance copy',
      );
      const target = path.join(acceptance, 'game/scenes/components/resource-bar.tscn');
      assert((await readFile(target, 'utf8')).includes('theme_override_constants/separation = 4'));
      const project = await invoke('create_session', { workspace: acceptance });
      await invoke('send_message', {
        id: project.id,
        text: 'In this isolated Rodent Heroes acceptance copy, apply the already authorized bounded presentation change now: in game/scenes/components/resource-bar.tscn change theme_override_constants/separation from 4 to 6. This Godot resource-only edit is exempt from the code graph requirement. Read the file, apply exactly that edit, then check git diff --check and git diff -- game/scenes/components/resource-bar.tscn. No planning turn or questions are needed. Do not edit other files, start the game, export, deploy, use subagents, network services or paid tools. Reply with the changed path and verification result.',
        options,
      });
      for (let i = 0; i < 900; i++) {
        const status = (await invoke('list_sessions', { search: '', offset: 0 })).find(
          (row) => row.id === project.id,
        )?.status;
        if (status === 'error')
          throw Error(
            'Rodent isolated native turn failed: ' +
              JSON.stringify((await invoke('get_messages', { id: project.id })).at(-1)),
          );
        if (status === 'completed') break;
        await sleep(200);
      }
      assert((await readFile(target, 'utf8')).includes('theme_override_constants/separation = 6'));
      const checks = await invoke('project_checks', { folder: acceptance });
      const reports = [];
      for (const check of checks.filter(
        (c) => c.id.startsWith('godot-assets:') || c.id.startsWith('godot-dotnet:'),
      )) {
        const report = await invoke('run_project_check', {
          folder: acceptance,
          id: check.id,
          command: check.command,
          cwd: check.cwd,
        });
        reports.push({ id: check.id, ...report });
        assert(report.ok, JSON.stringify(report));
      }
      results.push({
        project: 'Rodent Heroes isolated copy',
        workspace: acceptance,
        changed: 'game/scenes/components/resource-bar.tscn',
        reports,
      });
      console.log(JSON.stringify(results.at(-1)));
    }
    if (provider === 'gpt' && process.env.LUMEN_LIVE_ORCHESTRA === '1') {
      const workspace = path.join(data, 'orchestra-project');
      await mkdir(workspace);
      const git = (...args) =>
        execFileSync('git', args, { cwd: workspace, windowsHide: true, encoding: 'utf8' });
      await writeFile(
        path.join(workspace, 'README.md'),
        'Disposable isolated Orchestra acceptance. No external services.\n',
      );
      git('init', '-q');
      git('add', '.');
      git('-c', 'user.name=Test', '-c', 'user.email=test@localhost', 'commit', '-qm', 'Baseline');
      const prefs = await invoke('get_settings');
      await invoke('save_settings', {
        value: {
          ...prefs,
          orchestra: true,
          reviewRounds: 0,
          orchestraLimits: { maxConcurrent: 1, maxSubtasks: 2, quotaThresholds: {} },
        },
      });
      const s = await invoke('create_session', { workspace });
      const started = Date.now();
      const plan = await invoke('send_message', {
        id: s.id,
        text: 'Implement this already approved two-part architecture acceptance in this disposable project, using Orchestra with exactly two writing tasks: task one creates alpha.txt containing ALPHA_OK; task two creates beta.txt containing BETA_OK. The tasks have independent file ownership. Do not ask questions, request approval or return only a plan: execute both edits now under Autopilot. Do not modify README.md, access the network, run nested agents or add any other files. Verify the final contents locally. This request is the full specification; there are no missing product decisions.',
        options,
      });
      assert.equal(plan.strategy, 'orchestra');
      let state;
      for (let i = 0; i < 1800; i++) {
        const messages = await invoke('get_messages', { id: s.id });
        const activity = messages
          .flatMap((m) => m.activity)
          .filter((a) => a.kind === 'orchestra')
          .at(-1);
        if (activity) state = JSON.parse(activity.text);
        const status = (await invoke('list_sessions', { search: '', offset: 0 })).find(
          (row) => row.id === s.id,
        )?.status;
        if (status === 'error' || (status === 'completed' && state?.stage !== 'done'))
          throw Error('Real Orchestra stalled: ' + JSON.stringify(state));
        if (status === 'completed' && state?.stage === 'done') break;
        await sleep(200);
      }
      assert.equal(state?.stage, 'done');
      assert.equal(state.subtasks.length, 2);
      assert.equal((await readFile(path.join(workspace, 'alpha.txt'), 'utf8')).trim(), 'ALPHA_OK');
      assert.equal((await readFile(path.join(workspace, 'beta.txt'), 'utf8')).trim(), 'BETA_OK');
      assert.equal(git('worktree', 'list').trim().split(/\r?\n/).length, 1);
      results.push({
        orchestra: true,
        passed: true,
        autopilot: true,
        totalCap: 2,
        concurrencyCap: 1,
        elapsedMs: Date.now() - started,
      });
      console.log(JSON.stringify(results.at(-1)));
      await invoke('save_settings', { value: prefs });
    }
  }
  await mkdir(path.join(root, '.impeccable/review'), { recursive: true });
  await writeFile(
    path.join(root, '.impeccable/review/live-native-results.json'),
    JSON.stringify(results, null, 2),
  );
} finally {
  moliFixture.close();
  if (invoke) await invoke('quit_app').catch(() => {});
  if (browser) await browser.close().catch(() => {});
  await sleep(1000);
  if (child.exitCode === null) {
    try {
      execFileSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {
        windowsHide: true,
        stdio: 'ignore',
      });
    } catch {}
  }
  for (const link of links) await unlink(link).catch(() => {});
}
