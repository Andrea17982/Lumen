// Actual Windows WebView2 gallery, using disposable demonstration data only.
import { chromium } from 'playwright';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';

assert.equal(process.platform, 'win32', 'The native gallery targets Windows WebView2.');
const root = process.cwd();
const data = await mkdtemp(path.join(tmpdir(), 'lumen-release-gallery-'));
const workspace = path.join(data, 'Studio');
const output = path.join(root, 'docs/images');
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
await mkdir(path.join(workspace, '.lumen/screenshots'), { recursive: true });
await mkdir(output, { recursive: true });
await mkdir(path.join(data, 'cli'));
for (const provider of ['agy', 'gemini'])
  await writeFile(path.join(data, 'cli', `${provider}.cmd`), '@echo off\nexit /b 1\n');
await writeFile(
  path.join(data, 'settings.json'),
  JSON.stringify({
    repoContext: false,
    verifyChanges: false,
    orchestra: false,
    closeToTray: false,
  }),
);
const fixture = createServer((request, response) => {
  response.setHeader('Content-Type', 'text/html; charset=utf-8');
  response.end(
    '<html><head><title>Studio notes</title><style>body{font-family:Arial,sans-serif;background:#f5f8fc;color:#253349;margin:0;padding:48px}main{max-width:940px;margin:auto}small{color:#426c94}h1{font-size:44px;margin:18px 0}p{font-size:19px;line-height:1.7}section{background:white;padding:24px;margin-top:22px;border:1px solid #d8e2ef}h2{font-size:24px}a{color:#426c94}</style></head><body><main><small>STUDIO / PROJECT NOTES</small><h1>Build. Inspect. Continue.</h1><p>A local demonstration page captured with Moli.</p><section><h2>Keep the right context</h2><p>One project folder. Clear decisions. An inspectable result.</p><a href="#review">Read the review notes</a></section><section id="review"><h2>Ready for the next step</h2><p>Browser captures stay beside the conversation.</p></section></main></body></html>',
  );
});
await new Promise((resolve) => fixture.listen(0, '127.0.0.1', resolve));
const pageUrl = `http://127.0.0.1:${fixture.address().port}/`;
const ollama = createServer((request, response) => {
  response.setHeader('Content-Type', 'application/json');
  response.end(
    JSON.stringify({
      models: [
        { name: 'local-demo:latest', size: 2_000_000_000, capabilities: ['completion', 'tools'] },
      ],
    }),
  );
});
await new Promise((resolve) => ollama.listen(0, '127.0.0.1', resolve));
let moli, moliBrowser, app, browser, invoke;
try {
  moli = spawn(
    path.join(root, 'src-tauri/resources/moli/win32-x64/moli.exe'),
    [
      'serve',
      '--host',
      '127.0.0.1',
      '--port',
      '19389',
      '--layout',
      '--image',
      '--font',
      '--profile-dir',
      path.join(data, 'browser'),
    ],
    { windowsHide: true, stdio: 'ignore' },
  );
  for (let i = 0; i < 100; i++) {
    try {
      moliBrowser = await chromium.connectOverCDP('http://127.0.0.1:19389');
      break;
    } catch {
      await sleep(100);
    }
  }
  assert(moliBrowser, 'Moli did not start');
  const capture = await moliBrowser.contexts()[0].newPage();
  await capture.goto(pageUrl, { waitUntil: 'domcontentloaded' });
  await capture.screenshot({ path: path.join(workspace, '.lumen/screenshots/browser-demo.png') });
  await moliBrowser.close();
  moliBrowser = undefined;
  moli.kill();
  await writeFile(
    path.join(workspace, '.lumen/layout.json'),
    JSON.stringify({
      version: 1,
      width: 800,
      height: 600,
      blocks: [
        {
          id: 'nav',
          label: 'Navigation',
          role: 'sidebar',
          x: 16,
          y: 16,
          width: 168,
          height: 568,
          notes: 'Projects and conversations',
        },
        {
          id: 'body',
          label: 'Main view',
          role: 'content',
          x: 200,
          y: 16,
          width: 584,
          height: 460,
          notes: 'The primary reading surface',
        },
        {
          id: 'compose',
          label: 'Composer',
          role: 'form',
          x: 200,
          y: 492,
          width: 584,
          height: 92,
          notes: 'Request and working controls',
        },
      ],
    }),
  );
  app = spawn(path.join(root, 'src-tauri/target/debug/lumen.exe'), [], {
    cwd: root,
    windowsHide: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      LUMEN_TEST_DATA_DIR: data,
      WEBVIEW2_USER_DATA_FOLDER: path.join(data, 'webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:
        '--remote-debugging-port=9226 --force-device-scale-factor=1',
      OLLAMA_HOST: `http://127.0.0.1:${ollama.address().port}`,
      PATH: `${path.join(data, 'cli')};${path.join(root, 'tests/fixtures/cli')};${process.env.PATH}`,
    },
  });
  for (let i = 0; i < 100; i++) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9226');
      break;
    } catch {
      await sleep(100);
    }
  }
  assert(browser, 'Native app did not start');
  const page = browser
    .contexts()[0]
    .pages()
    .find((p) => !p.url().startsWith('devtools:'));
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.locator('h1').first().waitFor();
  invoke = (command, args = {}) =>
    page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
      command,
      args,
    });
  assert.equal((await invoke('diagnostics')).dataDir, data);
  const accounts = [];
  for (const [provider, name] of [
    ['claude', 'Claude Code'],
    ['gpt', 'Codex'],
    ['gemini', 'Gemini CLI'],
    ['antigravity', 'Antigravity'],
    ['ollama', 'Local Ollama'],
  ])
    accounts.push(await invoke('add_account', { provider, name }));
  const project = await invoke('add_workspace', { name: 'Studio', folder: workspace });
  const labels = ['Native workspace', 'A coordinated feature', 'Browse with Moli'];
  const sessions = [];
  for (const title of labels) {
    const session = await invoke('create_session', { workspace });
    await invoke('set_session_workspace', {
      id: session.id,
      workspaceId: project.id,
      folder: workspace,
    });
    sessions.push({ ...session, title });
  }
  const activity = [{ kind: 'workspace-root', text: workspace }];
  const hero = [
    { role: 'user', text: 'Help me work on this project without losing the thread.', activity },
    {
      role: 'assistant',
      text: '## One workspace, a clear next step\n\nKeep your selected provider and model, then choose how much coordination the task needs.\n\n- **Direct sessions** preserve the native workflow.\n- **Moli** handles research and page interaction.\n- **Orchestra** coordinates workers when you enable it.\n\nProject decisions, browser captures and verification stay close to the conversation.',
      activity: [
        ...activity,
        { kind: 'model', text: 'Claude · Opus · provider default reasoning' },
        {
          kind: 'child',
          text: JSON.stringify({
            id: 'demo-review',
            kind: 'snapshot',
            title: 'Read-only project review',
            status: 'completed',
            text: 'Demonstration child-agent summary: focused findings can be inspected without expanding the main answer.',
          }),
        },
      ],
    },
  ];
  const subtask = (id, title, account, files, dependsOn = []) => ({
    id,
    title,
    detail: 'Implement the owned change and verify its affected behavior.',
    files,
    dependsOn,
    difficulty: 'normal',
    writes: true,
    accountId: account.id,
    agent: account.provider === 'claude' ? 'Claude · Sonnet' : 'Codex · Coding model',
    model: '',
    effort: 'low',
    status: 'pending',
    output: '',
    startedAt: 0,
    endedAt: 0,
  });
  const state = {
    stage: 'plan',
    task: 'Build a small inventory editor',
    summary: 'Keep the API contract and interface work separate, then verify their integration.',
    questions: [],
    confirm: '',
    answers: '',
    orchestrator: accounts[0].id,
    orchestratorAgent: 'Claude · Opus',
    orchestratorModel: 'opus',
    effort: 'medium',
    readOnly: false,
    steps: [],
    lanes: [],
    planReview: '',
    subtasks: [
      subtask('s1', 'Define the data contract', accounts[0], ['src/catalog.ts']),
      subtask('s2', 'Build the editor view', accounts[1], ['src/Editor.tsx'], ['s1']),
      subtask('s3', 'Verify the complete flow', accounts[0], ['tests/editor.test.ts'], ['s2']),
    ],
  };
  const messages = [
    hero,
    [
      {
        role: 'user',
        text: 'Coordinate the inventory editor, with clear file ownership.',
        activity,
      },
      {
        role: 'assistant',
        text: '',
        activity: [...activity, { kind: 'orchestra', text: JSON.stringify(state) }],
      },
    ],
    [
      { role: 'user', text: 'Open the studio notes and capture the page for review.', activity },
      {
        role: 'assistant',
        text: 'Captured the local demonstration page with **Moli**. The complete PNG stays in this project, beside the conversation.',
        activity: [
          ...activity,
          { kind: 'browser', text: `Moli · browser_navigate\n${pageUrl}` },
          { kind: 'browser', text: 'Moli · browser_screenshot\nStudio notes' },
          {
            kind: 'image',
            text: JSON.stringify({
              id: 'demo-shot',
              path: '.lumen/screenshots/browser-demo.png',
              folder: workspace,
            }),
          },
        ],
      },
    ],
  ];
  const payload = path.join(data, 'gallery.json');
  await writeFile(payload, JSON.stringify({ sessions, messages }));
  execFileSync(
    'python',
    [
      '-c',
      'import sqlite3,json,sys,time,uuid; d=json.load(open(sys.argv[1],encoding="utf-8")); c=sqlite3.connect(sys.argv[2]); now=int(time.time()*1000);\nfor i,s in enumerate(d["sessions"]):\n c.execute("UPDATE sessions SET title=?,status=?,updated_at=?,access_mode=? WHERE id=?",(s["title"],"completed",now-i*1000,"file-edits" if i==1 else "read-only",s["id"]));\n for j,m in enumerate(d["messages"][i]): c.execute("INSERT INTO messages(id,session_id,role,text,activity,created_at) VALUES(?,?,?,?,?,?)",(str(uuid.uuid4()),s["id"],m["role"],m["text"],json.dumps(m["activity"]),now+j));\nc.commit()',
      payload,
      path.join(data, 'lumen.db'),
    ],
    { windowsHide: true },
  );
  await page.evaluate(() => {
    localStorage.setItem('lumen-theme', 'light');
    localStorage.setItem('lumen-language', 'en');
  });
  await page.reload();
  const select = async (name) => {
    await page
      .getByRole('button', { name: new RegExp(`^(?!Actions for).*${name}`) })
      .first()
      .click();
    await sleep(350);
  };
  await select(labels[0]);
  await page.locator('.prose').last().waitFor();
  await page.screenshot({ path: path.join(output, 'desktop.png') });
  await page.evaluate(() => localStorage.setItem('lumen-theme', 'dark'));
  await page.reload();
  await select(labels[0]);
  await page.screenshot({ path: path.join(output, 'dark.png') });
  await page.evaluate(() => localStorage.setItem('lumen-theme', 'light'));
  await page.reload();
  await select(labels[1]);
  await page.getByRole('region', { name: 'Task graph' }).waitFor();
  await page.screenshot({ path: path.join(output, 'orchestra-plan.png') });
  await select(labels[2]);
  const image = page.locator('.response-image img');
  await image.waitFor();
  assert(await image.evaluate((img) => img.complete && img.naturalWidth > 0));
  await page.screenshot({ path: path.join(output, 'moli-browser.png') });
  await select(labels[0]);
  await page.getByRole('button', { name: 'Project inspector', exact: true }).click();
  await page.getByRole('button', { name: 'Layout canvas', exact: true }).click();
  await page.getByText('Main view', { exact: true }).first().waitFor();
  await page.screenshot({ path: path.join(output, 'inspector-canvas.png') });
  await page.getByRole('button', { name: 'Close inspector', exact: true }).click();
  await page.getByRole('button', { name: 'Settings', exact: true }).first().click();
  await page.screenshot({ path: path.join(output, 'settings-simple.png') });
  await page.getByRole('button', { name: 'Advanced', exact: true }).click();
  await page.getByRole('tab', { name: 'General', exact: true }).click();
  await page.screenshot({ path: path.join(output, 'settings-advanced.png') });
  console.log(
    'Captured seven native 0.7 gallery views with isolated demonstration data and an actual Moli capture.',
  );
} finally {
  if (invoke) await invoke('quit_app').catch(() => {});
  if (browser) await browser.close().catch(() => {});
  if (moliBrowser) await moliBrowser.close().catch(() => {});
  if (moli && moli.exitCode === null) moli.kill();
  if (app && app.exitCode === null) app.kill();
  fixture.close();
  ollama.close();
}
