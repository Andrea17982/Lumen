// Windows debug-only test. PATH points to fixtures, app data to a fresh temp directory.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, realpath } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';

const root = process.cwd();
const testData = await mkdtemp(path.join(tmpdir(), 'lumen-native-test-'));
const workspace = path.join(testData, 'workspace');
await mkdir(workspace);
const child = spawn(path.join(root, 'src-tauri/target/debug/lumen.exe'), [], {
  cwd: root,
  windowsHide: true,
  stdio: 'pipe',
  env: {
    ...process.env,
    LUMEN_TEST_DATA_DIR: testData,
    WEBVIEW2_USER_DATA_FOLDER: path.join(testData, 'webview'),
    PATH: `${path.join(root, 'tests/fixtures/cli')};${process.env.PATH}`,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9224',
  },
});
let errors = '';
child.stderr.on('data', (chunk) => {
  errors += chunk.toString();
});
let browser;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
try {
  for (let i = 0; i < 100; i++) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9224');
      break;
    } catch {
      await sleep(200);
    }
  }
  assert(browser, `WebView2 did not expose its test endpoint: ${errors}`);
  const context = browser.contexts()[0];
  let page;
  for (let i = 0; i < 100; i++) {
    page = context.pages().find((p) => !p.url().startsWith('devtools:'));
    if (page && (await page.locator('h1').count())) break;
    await sleep(100);
  }
  assert(page, 'No Lumen webview');
  await page.locator('.welcome h1, h1').first().waitFor();
  const invoke = (command, args = {}) =>
    page.evaluate(async ({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
      command,
      args,
    });
  const diagnostics = await invoke('diagnostics');
  assert.equal(diagnostics.dataDir, testData);
  assert(diagnostics.claude && diagnostics.gpt);
  const gpt = await invoke('add_account', { name: 'Offline GPT fixture', provider: 'gpt' });
  const claude = await invoke('add_account', {
    name: 'Offline Claude fixture',
    provider: 'claude',
  });
  assert.equal(await invoke('account_status', { id: gpt.id }), true);
  assert.equal(await invoke('account_status', { id: claude.id }), true);
  const catalog = await invoke('list_models', { id: gpt.id, refresh: true });
  assert(catalog.find((m) => m.id === 'gpt-fixture').efforts.includes('ultra'));
  const claudeCatalog = await invoke('list_models', { id: claude.id, refresh: true });
  assert(claudeCatalog.find((m) => m.id === 'opus').efforts.includes('max'));
  await invoke('rename_account', { id: gpt.id, name: 'Renamed GPT fixture' });
  assert(
    (await invoke('list_accounts')).some(
      (a) => a.id === gpt.id && a.name === 'Renamed GPT fixture',
    ),
  );
  const options = {
    mode: 'gpt',
    accountId: gpt.id,
    model: null,
    effort: 'low',
    accessMode: 'read-only',
  };
  const officialQuota = await invoke('refresh_usage', { id: gpt.id });
  assert.equal(officialQuota.source, 'codex-cli');
  assert.equal(officialQuota.windows.length, 2);
  async function waitSession(id, status) {
    for (let i = 0; i < 200; i++) {
      const rows = await invoke('list_sessions', { search: '', offset: 0 });
      const s = rows.find((s) => s.id === id);
      if (s?.status === status) return s;
      if (s?.status === 'error')
        throw new Error(JSON.stringify(await invoke('get_messages', { id })));
      await sleep(100);
    }
    throw new Error(`Session did not reach ${status}`);
  }
  const s = await invoke('create_session', { workspace });
  const named = await invoke('add_workspace', {
    name: 'Native fixture workspace',
    folder: workspace,
  });
  const subfolder = path.join(workspace, 'subfolder');
  await mkdir(subfolder);
  const linked = await invoke('set_session_workspace', {
    id: s.id,
    workspaceId: named.id,
    folder: subfolder,
  });
  assert.equal(linked.workspaceId, named.id);
  assert(linked.workspace.endsWith('subfolder'));
  await assert.rejects(() =>
    invoke('send_message', {
      id: s.id,
      text: 'Invalid effort fixture',
      options: { ...options, model: 'gpt-fast-fixture', effort: 'ultra' },
    }),
  );
  await invoke('send_message', { id: s.id, text: 'UI fixture: review this sample', options });
  await waitSession(s.id, 'completed');
  const messages = await invoke('get_messages', { id: s.id });
  assert.equal(messages.length, 2);
  assert(messages[1].text.includes('offline UI fixture'));
  assert(messages[1].activity.some((a) => a.kind === 'file'));
  const cwdEvent = messages[1].activity.find(
    (a) => a.kind === 'tool' && a.text.startsWith('fixture-only: working directory\n'),
  );
  assert(cwdEvent, 'The fixture must report its working directory');
  const actualFolder = cwdEvent.text.split('\n').slice(1).join('\n');
  assert.equal(
    (await realpath(actualFolder)).toLowerCase(),
    (await realpath(subfolder)).toLowerCase(),
    'The child must actually run inside the conversation folder (including Windows short/long paths)',
  );
  await invoke('update_session', { id: s.id, title: 'Offline UI fixture', pinned: true });
  const claudeSession = await invoke('create_session', { workspace });
  await invoke('send_message', {
    id: claudeSession.id,
    text: 'Claude stream fixture',
    options: { ...options, mode: 'claude', accountId: claude.id },
  });
  await waitSession(claudeSession.id, 'completed');
  const streamed = await invoke('get_messages', { id: claudeSession.id });
  assert.equal(streamed[1].text, messages[1].text);
  for (const [text, model, effort] of [
    ['Test', 'gpt-luna-fixture', 'low'],
    ['Traduci questa email', 'gpt-luna-fixture', 'low'],
    ['Correggi un errore CSS', 'gpt-terra-fixture', 'low'],
    ['Trova un bug intermittente di concorrenza', 'gpt-terra-fixture', 'medium'],
    ['Progetta una migrazione del database', 'opus', 'low'],
    ["Controlla la sicurezza dell'autenticazione", 'opus', 'low'],
  ]) {
    const run = await invoke('create_session', { workspace });
    const plan = await invoke('send_message', {
      id: run.id,
      text,
      options: { ...options, mode: 'auto', accountId: null },
    });
    assert.equal(plan.model, model, text);
    assert.equal(plan.effort, effort, text);
    await waitSession(run.id, 'completed');
  }
  const balanced = await invoke('create_session', { workspace });
  // Native questions must work in Autopilot without granting permissions or starting a new turn.
  for (const account of [gpt, claude]) {
    const question = await invoke('create_session', { workspace });
    await invoke('send_message', {
      id: question.id,
      text: `QUESTION_FIXTURE ${account.provider}`,
      options: {
        ...options,
        mode: account.provider,
        accountId: account.id,
        accessMode: 'autopilot',
      },
    });
    let request;
    for (let i = 0; i < 150; i++) {
      const messages = await invoke('get_messages', { id: question.id });
      const event = messages.flatMap((m) => m.activity).find((a) => a.kind === 'interaction');
      if (event) {
        request = JSON.parse(event.text);
        break;
      }
      await sleep(100);
    }
    assert(request, 'The native question must be persisted while the CLI waits');
    assert((await invoke('pending_questions')).includes(request.id));
    await page.reload();
    await page
      .getByRole('button', {
        name: new RegExp('^(?!Actions for).*QUESTION_FIXTURE ' + account.provider),
      })
      .click();
    await page.getByRole('radio', { name: /Native/ }).check();
    await page.getByRole('button', { name: 'Send answers', exact: true }).click();
    await waitSession(question.id, 'completed');
    assert.equal(
      (await readFile(path.join(workspace, `answer-${account.provider}.txt`), 'utf8')).trim(),
      'Native',
    );
    assert(!(await invoke('pending_questions')).includes(request.id));
    await assert.rejects(
      invoke('answer_question', {
        id: request.id,
        answers: { [request.questions[0].id]: ['Custom'] },
      }),
    );
  }
  const balancedPlan = await invoke('send_message', {
    id: balanced.id,
    text: 'Test',
    options: { ...options, mode: 'auto', accountId: null, balanceUsage: true },
  });
  assert.equal(balancedPlan.accountId, claude.id, 'Weekly quota near reset should be used first');
  assert.equal(balancedPlan.model, 'haiku');
  await waitSession(balanced.id, 'completed');
  for (const [text, strategy] of [
    ['Refactor security architecture in production', 'review'],
    ['Compare alternatives', 'single'],
  ]) {
    const run = await invoke('create_session', { workspace });
    const plan = await invoke('send_message', {
      id: run.id,
      text,
      options: { ...options, mode: 'auto', accountId: null },
    });
    assert.equal(plan.strategy, strategy);
    await waitSession(run.id, 'completed');
    const transcript = await invoke('get_messages', { id: run.id });
    if (strategy === 'review')
      assert(
        transcript[1].activity.some(
          (a) => a.kind === (strategy === 'review' ? 'review' : 'perspectives'),
        ),
      );
  }
  const queued = [];
  for (let i = 0; i < 5; i++) {
    const run = await invoke('create_session', { workspace });
    queued.push(run.id);
    await invoke('send_message', { id: run.id, text: 'SLOW_FIXTURE', options });
  }
  const states = await invoke('list_sessions', { search: '', offset: 0 });
  assert(states.some((s) => queued.includes(s.id) && s.status === 'waiting'));
  for (const id of queued) await invoke('cancel_run', { id });
  for (const id of queued) await waitSession(id, 'interrupted');
  await page.reload();
  await page.getByRole('button', { name: /^(?!Actions for).*Offline UI fixture/ }).click();
  await page.getByText('This is an offline UI fixture', { exact: false }).waitFor();
  await mkdir(path.join(root, '.impeccable/review'), { recursive: true });
  await page.screenshot({ path: '.impeccable/review/native-chat-fixture.png' });
  await page.locator('.activity > summary').click();
  await page.screenshot({ path: '.impeccable/review/native-activity-fixture.png' });
  await page.getByRole('button', { name: 'GPT', exact: true }).click();
  await page.getByRole('button', { name: 'Advanced provider options' }).click();
  await page.getByRole('combobox', { name: 'Model', exact: true }).selectOption('gpt-fast-fixture');
  assert.deepEqual(
    await page
      .getByRole('combobox', { name: 'Reasoning effort' })
      .locator('option')
      .evaluateAll((options) => options.map((o) => o.value)),
    ['', 'low', 'medium'],
  );
  for (let i = 0; i < 4; i++) {
    if (await page.getByRole('button', { name: 'Permissions: Autopilot' }).count()) break;
    await page.getByRole('button', { name: /^Permissions: / }).click();
  }
  await page.getByRole('button', { name: 'Permissions: Autopilot' }).waitFor();
  await page.getByRole('button', { name: 'Rename selected account' }).click();
  await page.getByRole('textbox', { name: 'Account name' }).fill('Composer renamed account');
  await page.getByRole('button', { name: 'Save account name' }).click();
  await page
    .getByRole('combobox', { name: 'Account', exact: true })
    .getByRole('option', { name: 'Composer renamed account' })
    .waitFor({ state: 'attached' });
  await page.screenshot({ path: '.impeccable/review/native-provider-controls.png' });
  await page.getByRole('button', { name: 'Manage accounts' }).click();
  await page.getByRole('button', { name: 'Rename Composer renamed account' }).click();
  await page.getByRole('textbox', { name: 'Account name' }).fill('Settings renamed account');
  await page.getByRole('button', { name: 'Save account name' }).click();
  await page.getByRole('button', { name: 'Remove Settings renamed account' }).waitFor();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.getByRole('dialog').waitFor({ state: 'detached' });
  assert(
    (await invoke('list_accounts')).some(
      (a) => a.id === gpt.id && a.name === 'Settings renamed account',
    ),
  );
  const autopilot = await invoke('create_session', { workspace });
  await invoke('send_message', {
    id: autopilot.id,
    text: 'Offline autopilot flags fixture',
    options: { ...options, accessMode: 'autopilot' },
  });
  await waitSession(autopilot.id, 'completed');

  // 0.3: checkpoints + Rewind, Both mode in isolated worktrees, rate-limit fallback,
  // project context, compact handoff, settings and the local API.
  const repo = path.join(testData, 'repo with spaces');
  await mkdir(repo);
  const git = (...args) => execFileSync('git', args, { cwd: repo, encoding: 'utf8' });
  git('init', '-q');
  await writeFile(path.join(repo, 'shared.txt'), 'original\n');
  await writeFile(path.join(repo, '.gitignore'), 'dist/\n');
  git('add', '.');
  git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'init');
  const edits = { ...options, accessMode: 'file-edits' };
  const rw = await invoke('create_session', { workspace: repo });
  await invoke('send_message', { id: rw.id, text: 'WRITE_FIXTURE first turn', options: edits });
  await waitSession(rw.id, 'completed');
  assert.match(await readFile(path.join(repo, 'shared.txt'), 'utf8'), /edited by/);
  const firstUser = (await invoke('get_messages', { id: rw.id }))[0];
  assert(
    firstUser.activity.some((a) => a.kind === 'checkpoint'),
    'write turns are checkpointed',
  );
  const preview = await invoke('rewind_preview', { session: rw.id, message: firstUser.id });
  assert(preview.restorable && preview.files.length === 2, JSON.stringify(preview));
  assert.equal(await invoke('rewind', { session: rw.id, message: firstUser.id }), firstUser.text);
  assert.equal(await readFile(path.join(repo, 'shared.txt'), 'utf8'), 'original\n');
  assert.equal((await invoke('get_messages', { id: rw.id })).length, 0);
  assert.equal(git('status', '--porcelain').trim(), '', 'rewind leaves the repo clean');

  const both = await invoke('create_session', { workspace: repo });
  const bothPlan = await invoke('send_message', {
    id: both.id,
    text: 'WRITE_FIXTURE in both lanes',
    options: { ...edits, mode: 'both' },
  });
  assert.equal(bothPlan.strategy, 'broadcast');
  await waitSession(both.id, 'completed');
  const lanes = (await invoke('get_messages', { id: both.id })).filter(
    (m) => m.role === 'assistant',
  );
  assert.equal(lanes.length, 2);
  assert(lanes.every((m) => m.activity.some((a) => a.kind === 'worktree')));
  const variants = await invoke('visual_variants', { session: both.id });
  assert.equal(variants.length, 3);
  const rendered = [];
  for (const variant of variants.filter((v) => v.id !== 'project')) {
    await mkdir(path.join(variant.path, 'dist'), { recursive: true });
    await writeFile(path.join(variant.path, 'dist/index.html'), `<h1>${variant.id}</h1>`);
    const preview = await invoke('start_variant_preview', {
      session: both.id,
      variant: variant.id,
      directory: 'dist',
    });
    assert((await (await fetch(preview.url)).text()).includes(variant.id));
    rendered.push(preview);
    assert.equal((await fetch(preview.url + '.env')).status, 404);
  }
  for (const preview of rendered) {
    await invoke('stop_variant_preview', { id: preview.id });
    await assert.rejects(fetch(preview.url));
  }
  await assert.rejects(
    invoke('start_variant_preview', { session: both.id, variant: 'project', directory: '../' }),
  );
  assert.equal(await readFile(path.join(repo, 'shared.txt'), 'utf8'), 'original\n');
  const gptLane = lanes.find((m) =>
    m.activity.some((a) => a.kind === 'lane' && a.text === 'codex'),
  );
  await invoke('resolve_lanes', { session: both.id, message: gptLane.id, apply: true });
  assert.match(await readFile(path.join(repo, 'shared.txt'), 'utf8'), /edited by gpt/);
  assert.equal(git('worktree', 'list').trim().split('\n').length, 1, 'lanes are removed');
  const merged = await invoke('create_session', { workspace: repo });
  await invoke('send_message', {
    id: merged.id,
    text: 'WRITE_FIXTURE manual merge probe',
    options: { ...edits, mode: 'both' },
  });
  await waitSession(merged.id, 'completed');
  const mergeMessage = (await invoke('get_messages', { id: merged.id })).find((m) =>
    m.activity.some((a) => a.kind === 'worktree'),
  );
  const mergePreview = await invoke('preview_manual_merge', {
    session: merged.id,
    message: mergeMessage.id,
  });
  const indexBefore = git('diff', '--cached');
  await invoke('apply_manual_merge', {
    session: merged.id,
    preview: mergePreview.id,
    edits: mergePreview.files.map((f) => ({
      path: f.path,
      side: f.path === 'shared.txt' ? 'manual' : 'left',
      text: f.path === 'shared.txt' ? 'reviewed manual result\n' : null,
    })),
  });
  assert.equal(
    (await readFile(path.join(repo, 'shared.txt'), 'utf8')).trim(),
    'reviewed manual result',
  );
  assert.equal(git('diff', '--cached'), indexBefore, 'manual merge preserves the user index');
  await assert.rejects(
    invoke('apply_manual_merge', { session: merged.id, preview: mergePreview.id, edits: [] }),
  );

  const game = path.join(testData, 'game-controls');
  await mkdir(game);
  const server = { command: 'python', args: [path.join(root, 'tests/fixtures/godot-mcp.py')] };
  await writeFile(
    path.join(game, '.mcp.json'),
    JSON.stringify({ mcpServers: { 'godot-fixture': server } }),
  );
  await assert.rejects(invoke('godot_visual_action', { folder: game, action: 'play', scene: '' }));
  await invoke('set_godot_visual_settings', {
    folder: game,
    value: { enabled: true, server: 'godot-fixture' },
  });
  const connected = await invoke('godot_visual_action', {
    folder: game,
    action: 'discover',
    scene: '',
  });
  assert(connected.available.includes('play'));
  await invoke('godot_visual_action', { folder: game, action: 'play', scene: '' });
  await invoke('godot_visual_action', { folder: game, action: 'reload', scene: '' });
  await invoke('godot_visual_action', { folder: game, action: 'stop', scene: '' });
  const captured = await invoke('godot_visual_action', {
    folder: game,
    action: 'capture',
    scene: '',
  });
  assert.equal(captured.screenshot.width, 1);
  assert(
    (
      await invoke('response_image_data', { folder: game, href: captured.screenshot.path })
    ).startsWith('data:image/'),
  );
  const log = await readFile(path.join(game, 'visual-action-log.txt'), 'utf8');
  await writeFile(
    path.join(game, '.mcp.json'),
    JSON.stringify({
      mcpServers: { 'godot-fixture': { ...server, env: { LUMEN_TEST_EDITOR_ROOT: workspace } } },
    }),
  );
  await assert.rejects(invoke('godot_visual_action', { folder: game, action: 'play', scene: '' }));
  assert.equal(
    await readFile(path.join(game, 'visual-action-log.txt'), 'utf8'),
    log,
    'a different connected editor must receive no action',
  );

  const limited = await invoke('create_session', { workspace });
  await invoke('send_message', {
    id: limited.id,
    text: 'RATE_FIXTURE please',
    options: { ...options, mode: 'claude', accountId: claude.id },
  });
  await waitSession(limited.id, 'completed');
  const fell = (await invoke('get_messages', { id: limited.id }))[1];
  assert(
    fell.activity.some((a) => a.kind === 'fallback'),
    JSON.stringify(fell.activity),
  );
  assert(fell.text.includes('offline UI fixture'));
  const health = await invoke('provider_health');
  assert.equal(health.find((h) => h.accountId === claude.id).state, 'cooldown');

  await invoke('save_project_context', { folder: repo, notes: 'Use tabs.\nverify: off' });
  const ctx = await invoke('project_context', { folder: repo });
  assert(ctx.git && ctx.notes.includes('Use tabs') && ctx.files.some((f) => f.path === 'LUMEN.md'));
  const compacted = await invoke('compact_session', { id: both.id });
  const handoff = await invoke('get_messages', { id: compacted.id });
  assert(handoff[0].text.includes('Handoff') && handoff[0].text.includes('Goal'));

  const prefs = await invoke('get_settings');
  assert.equal(await invoke('capture_hotkey_registered'), false);
  await invoke('save_settings', { value: { ...prefs, captureHotkey: true } });
  assert.equal(await invoke('capture_hotkey_registered'), true);
  assert.deepEqual(
    await invoke('list_screenshots', { folder: repo }),
    [],
    'registration must not capture',
  );
  await invoke('reset_settings');
  assert.equal(await invoke('capture_hotkey_registered'), false);
  assert(
    (await invoke('list_accounts')).some((a) => a.id === gpt.id),
    'reset retains accounts',
  );

  // 0.4: Ollama through its official CLI, then Orchestra end to end on fixtures.
  const local = await invoke('add_account', { name: 'Local fixture', provider: 'ollama' });
  const ollama = await invoke('ollama_status');
  assert(ollama.installed && ollama.running && ollama.models.length === 2, JSON.stringify(ollama));
  assert.equal(ollama.selected, 'qwen-fixture:7b', 'the largest model is chosen');
  assert.equal(await invoke('account_status', { id: local.id }), true);
  const localModels = await invoke('list_models', { id: local.id, refresh: true });
  assert.deepEqual(
    localModels.map((m) => m.id),
    ['qwen-fixture:7b'],
  );
  await invoke('save_settings', { value: { ...prefs, orchestra: true } });
  const orc = await invoke('create_session', { workspace: repo });
  const orcPlan = await invoke('send_message', {
    id: orc.id,
    text: 'Implement the authentication architecture for the API and the settings UI',
    options: { ...edits, mode: 'auto', accountId: null, model: null, effort: '' },
  });
  assert.equal(orcPlan.strategy, 'orchestra');
  await waitSession(orc.id, 'completed');
  const lastState = async () => {
    const all = await invoke('get_messages', { id: orc.id });
    const m = all.at(-1);
    return [m, JSON.parse(m.activity.find((a) => a.kind === 'orchestra').text)];
  };
  let [asked, askedState] = await lastState();
  assert.equal(askedState.stage, 'questions');
  assert.equal(askedState.questions[0].recommended, 'orchestra-s1.txt and orchestra-s2.txt');
  const orcOptions = { ...edits, mode: 'auto', accountId: null, model: null, effort: '' };
  await invoke('orchestra_answer', {
    id: orc.id,
    message: asked.id,
    answers: [],
    skip: true,
    options: orcOptions,
  });
  await waitSession(orc.id, 'completed');
  let [planned, plannedState] = await lastState();
  assert.equal(plannedState.stage, 'plan', JSON.stringify(plannedState));
  assert.equal(plannedState.subtasks.length, 2);
  assert.deepEqual(plannedState.subtasks[1].dependsOn, ['s1']);
  assert(plannedState.subtasks.every((t) => t.accountId && t.accountId !== local.id));
  await invoke('orchestra_approve', {
    id: orc.id,
    message: planned.id,
    edits: [],
    options: orcOptions,
  });
  await waitSession(orc.id, 'completed');
  const [, doneState] = await lastState();
  assert.equal(doneState.stage, 'done', JSON.stringify(doneState));
  assert(doneState.subtasks.every((t) => t.status === 'done'));
  assert.match(await readFile(path.join(repo, 'orchestra-s1.txt'), 'utf8'), /by /);
  assert.match(await readFile(path.join(repo, 'orchestra-s2.txt'), 'utf8'), /by /);
  assert.equal(
    git('worktree', 'list').trim().split(/\r?\n/).length,
    1,
    'Orchestra lanes are removed',
  );
  const approvedUser = (await invoke('get_messages', { id: orc.id })).find(
    (m) => m.role === 'user' && m.text === 'Approved the plan.',
  );
  assert(
    approvedUser.activity.some((a) => a.kind === 'checkpoint'),
    'Orchestra is rewindable',
  );
  await invoke('save_settings', { value: { ...prefs, orchestra: false } });

  if (process.env.LUMEN_REAL_COMPILER === '1') {
    const compiler = path.join(testData, 'compiler probe with spaces');
    await mkdir(path.join(compiler, '.lumen'), { recursive: true });
    await writeFile(
      path.join(compiler, 'Probe.csproj'),
      '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>',
    );
    await writeFile(
      path.join(compiler, 'Probe.cs'),
      'class Probe { int n = "intentional type error"; }',
    );
    await writeFile(
      path.join(compiler, '.lumen/checks.json'),
      JSON.stringify([
        { name: 'Compiler source links', command: 'dotnet build --nologo', cwd: '.' },
      ]),
    );
    const check = (await invoke('project_checks', { folder: compiler })).find(
      (c) => c.label === 'Compiler source links',
    );
    assert(check);
    const report = await invoke('run_project_check', {
      folder: compiler,
      id: check.id,
      command: check.command,
      cwd: check.cwd,
    });
    assert.equal(report.ok, false);
    assert(report.output.includes('CS0029'));
    assert(
      report.links.some((l) => l.file === 'Probe.cs' && l.line === 1),
      JSON.stringify(report),
    );
    await writeFile('.impeccable/review/compiler-probe.json', JSON.stringify(report, null, 2));
  }

  // Seed only this harness's isolated database to measure a genuinely long native transcript.
  const long = await invoke('create_session', { workspace });
  await invoke('update_session', {
    id: long.id,
    title: 'Native transcript benchmark',
    pinned: false,
  });
  execFileSync(
    'python',
    [
      '-c',
      'import sqlite3,sys,json; c=sqlite3.connect(sys.argv[1]); c.executemany("INSERT INTO messages VALUES(?,?,?,?,?,?)",[("bench-"+str(i),sys.argv[2],"assistant" if i%2 else "user","Recorded message "+str(i)+"\\n\\n```ts\\nconst result = 42;\\n```",json.dumps([]),i) for i in range(1000)]); c.commit()',
      path.join(testData, 'lumen.db'),
      long.id,
    ],
    { windowsHide: true },
  );
  await page.reload();
  const target = page.getByRole('button', {
    name: /^(?!Actions for).*Native transcript benchmark/,
  });
  await target.waitFor();
  const started = performance.now();
  await target.click();
  await page.locator('.message').nth(999).waitFor();
  const loadMs = performance.now() - started;
  const frames = await page.evaluate(async () => {
    const el = document.querySelector('.conversation');
    const intervals = [];
    let last = performance.now();
    for (let i = 0; i < 60; i++) {
      await new Promise(requestAnimationFrame);
      const now = performance.now();
      intervals.push(now - last);
      last = now;
      if (el) el.scrollTop = (i % 2) * el.scrollHeight;
    }
    return intervals;
  });
  await page.getByRole('button', { name: 'Project inspector', exact: true }).click();
  await page.getByRole('button', { name: 'Layout canvas', exact: true }).click();
  await page.getByRole('button', { name: 'Add block' }).click();
  await page.screenshot({ path: '.impeccable/review/native-inspector-canvas.png' });
  await writeFile(
    '.impeccable/review/native-performance.json',
    JSON.stringify(
      {
        messages: 1000,
        loadMs,
        frameP50Ms: [...frames].sort((a, b) => a - b)[30],
        frameMaxMs: Math.max(...frames),
        note: 'Synthetic local transcript in Windows WebView2, no inference. Frame intervals include display scheduling.',
      },
      null,
      2,
    ),
  );

  await writeFile(
    '.impeccable/review/native-results.json',
    JSON.stringify(
      {
        passed: true,
        tests: [
          'isolated app data',
          'account metadata and official status command adapter',
          'session persistence',
          'Codex JSONL',
          'Claude partial streaming',
          'writer-reviewer',
          'parallel synthesis',
          'queue and cancellation',
          'native chat rendering',
          'ollama local provider',
          'orchestra questions, plan, parallel worktrees and integration',
        ],
        note: 'Offline CLI fixtures only; no provider accounts or inference were used.',
      },
      null,
      2,
    ),
  );
  // Tray: with the setting on, closing hides the window and Lumen keeps running;
  // Quit from the tray menu exits. The one-time notice is pre-acknowledged here.
  const trayPrefs = await invoke('get_settings');
  await invoke('save_settings', {
    value: { ...trayPrefs, closeToTray: true, trayHintShown: true },
  });
  await invoke('plugin:window|close', { label: 'main' });
  await sleep(1500);
  assert.equal(child.exitCode, null, 'closing with the tray on keeps Lumen running');
  assert.equal(
    await invoke('plugin:window|is_visible', { label: 'main' }),
    false,
    'the window is hidden',
  );
  const exited = new Promise((resolve) => child.once('exit', resolve));
  // The page goes away while the call is in flight, which rejects it: expected.
  await invoke('quit_app').catch(() => {});
  await Promise.race([exited, sleep(10000)]);
  assert.notEqual(child.exitCode, null, 'Quit exits Lumen');
  console.log('Native offline checks passed. Temporary test data: ' + testData);
} finally {
  if (browser) await browser.close().catch(() => {});
  if (!child.killed) child.kill();
}
