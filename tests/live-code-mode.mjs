// Explicit opt-in T3: Code Mode on/off through the official subscription CLIs (Claude Code, Codex).
// Uses each CLI's existing Lumen account directory via a temporary junction; no token is read or
// copied. Disposable workspaces and local fixtures only. Raw rows go to LUMEN_BENCH_OUTPUT (JSONL).
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
  appendFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { chromium } from 'playwright';
if (process.env.LUMEN_LIVE_CODE_MODE !== '1')
  throw Error('Set LUMEN_LIVE_CODE_MODE=1 to authorize live CLI turns.');
const output = process.env.LUMEN_BENCH_OUTPUT || 'reports/bench/live-code-mode.jsonl';
const seeds = Number(process.env.LUMEN_BENCH_SEEDS || 3);
const providers = (process.env.LUMEN_LIVE_PROVIDERS || 'claude,gpt').split(',');
const only = process.env.LUMEN_BENCH_CASES?.split(',');
const original =
  process.env.LUMEN_LIVE_ACCOUNT_ROOT ||
  path.join(process.env.APPDATA, 'io.github.andrea17982.lumen');
const accounts = JSON.parse(
  execFileSync(
    'python',
    [
      '-c',
      'import sqlite3,sys,json; c=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True); print(json.dumps(c.execute("SELECT provider,id FROM accounts").fetchall()))',
      path.join(original, 'lumen.db'),
    ],
    { encoding: 'utf8', windowsHide: true },
  ),
).map(([provider, id]) => ({ provider, id }));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Moli fixture: 30 items, every fourth from the second SOLD.
const items = Array.from(
  { length: 30 },
  (_, i) => '<li>Item ' + (i + 1) + ' ' + (i % 4 === 1 ? 'SOLD' : 'available') + '</li>',
).join('');
const site = createServer((_q, r) => {
  r.setHeader('content-type', 'text/html');
  r.end('<html><body><h1>Stock</h1><ul>' + items + '</ul></body></html>');
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const stock = 'http://127.0.0.1:' + site.address().port + '/stock';

// Many-tool MCP fixture shaped like the official servers; two tools carry the task data.
const families = {
  github: [
    'list_issues',
    'get_issue',
    'create_issue',
    'update_issue',
    'add_issue_comment',
    'search_issues',
    'list_pull_requests',
    'get_pull_request',
    'create_pull_request',
    'merge_pull_request',
    'list_commits',
    'get_file_contents',
    'create_branch',
    'search_code',
    'search_repositories',
    'fork_repository',
  ],
  filesystem: [
    'read_file',
    'read_multiple_files',
    'write_file',
    'edit_file',
    'create_directory',
    'list_directory',
    'directory_tree',
    'move_file',
    'search_files',
    'get_file_info',
  ],
  browser: [
    'navigate',
    'click',
    'fill',
    'select',
    'hover',
    'press_key',
    'screenshot',
    'snapshot',
    'evaluate',
    'wait_for',
    'tabs',
    'close',
  ],
  sqlite: [
    'read_query',
    'write_query',
    'create_table',
    'list_tables',
    'describe_table',
    'append_insight',
  ],
  memory: [
    'create_entities',
    'create_relations',
    'add_observations',
    'delete_entities',
    'delete_observations',
    'delete_relations',
    'read_graph',
    'search_nodes',
    'open_nodes',
  ],
  git: [
    'status',
    'diff_unstaged',
    'diff_staged',
    'diff',
    'commit',
    'add',
    'reset',
    'log',
    'create_branch',
    'checkout',
    'show',
  ],
};
const tools = Object.entries(families).flatMap(([f, ns]) =>
  ns.map((n) => ({
    name: f + '_' + n,
    description: n.replaceAll('_', ' ') + ' (' + f + ')',
    inputSchema: {
      type: 'object',
      properties: {
        owner: { type: 'string' },
        repo: { type: 'string' },
        path: { type: 'string' },
        query: { type: 'string' },
      },
    },
  })),
);
const issues = JSON.stringify([
  {
    number: 41,
    title: 'Docs typo on pricing page',
    labels: ['docs'],
    body: "The word 'monthly' is misspelled.",
  },
  {
    number: 57,
    title: 'Dates render one day early',
    labels: ['regression', 'bug'],
    body: 'Since the timezone refactor, the crash comes from src/date/format.ts.',
  },
  {
    number: 63,
    title: 'Add dark mode toggle',
    labels: ['enhancement'],
    body: 'Requested by several users.',
  },
]);
const source =
  "import { zone } from './zone';\n\nexport function formatInstant(value: Date): string {\n  return zone(value).toISOString().slice(0, 10);\n}\n";
const mcpServer = `import{createInterface}from'node:readline';const tools=${JSON.stringify(tools)};for await(const line of createInterface({input:process.stdin})){const v=JSON.parse(line);let result={};if(v.method==='initialize')result={protocolVersion:'2025-06-18',serverInfo:{name:'acme',version:'1'},capabilities:{tools:{}}};else if(v.method==='tools/list')result={tools};else if(v.method==='tools/call'){const n=v.params.name,a=v.params.arguments||{};const text=n==='github_list_issues'&&a.repo==='web'?${JSON.stringify(issues)}:(n==='filesystem_read_file'||n==='github_get_file_contents')&&String(a.path).endsWith('src/date/format.ts')?${JSON.stringify(source)}:'No results';result={content:[{type:'text',text}]};}if(v.id!==undefined)console.log(JSON.stringify({jsonrpc:'2.0',id:v.id,result}));}`;

const slugCheck =
  "const f=require('./lib.js');const a=require('node:assert');a.equal(f('Hello World'),'hello-world');a.equal(f('  A--B  '),'a-b');a.equal(f('Ünïcode & Co.'),'n-code-co');a.equal(f('x'),'x');console.log('ok');";
const cases = {
  A: {
    prompt:
      'Change the value alpha to beta in every file item-00.txt through item-19.txt. Preserve the other line in each file.',
    async prepare(w) {
      for (let i = 0; i < 20; i++)
        await writeFile(
          path.join(w, 'item-' + String(i).padStart(2, '0') + '.txt'),
          'value=alpha\nkeep=' + String(i).padStart(2, '0') + '\n',
        );
    },
    async judge(w) {
      for (let i = 0; i < 20; i++) {
        const t = await readFile(
          path.join(w, 'item-' + String(i).padStart(2, '0') + '.txt'),
          'utf8',
        );
        if (t.replace(/\r/g, '') !== 'value=beta\nkeep=' + String(i).padStart(2, '0') + '\n')
          return false;
      }
      return true;
    },
  },
  F: {
    prompt:
      'Implement lib.js so that module.exports is a function that turns a string into a lowercase slug: runs of characters other than a-z and 0-9 become one hyphen, with no leading or trailing hyphen. check.js holds the tests. Make node check.js pass, then answer done.',
    async prepare(w) {
      await writeFile(path.join(w, 'check.js'), slugCheck);
      await writeFile(path.join(w, 'lib.js'), "module.exports=()=>{throw new Error('todo')};\n");
    },
    async judge(w) {
      try {
        execFileSync(process.execPath, ['check.js'], {
          cwd: w,
          stdio: 'ignore',
          windowsHide: true,
        });
        return true;
      } catch {
        return false;
      }
    },
  },
  M: {
    prompt:
      'Open ' +
      stock +
      ' with the Moli browser tools, read the page and count the list items marked SOLD. Answer with the integer only.',
    answer: '8',
  },
  D: {
    prompt:
      'In the GitHub repository acme/web, find the open issue labelled regression. Its body names a source file. Read that file with the acme filesystem tools and answer with the name of the function it exports, alone.',
    async prepare(w) {
      await writeFile(path.join(w, 'acme-mcp.mjs'), mcpServer);
      await writeFile(
        path.join(w, '.mcp.json'),
        JSON.stringify({
          mcpServers: { acme: { command: process.execPath, args: [path.join(w, 'acme-mcp.mjs')] } },
        }),
      );
    },
    answer: 'formatInstant',
  },
};
const root = process.cwd();
const data = await mkdtemp(path.join(tmpdir(), 'lumen-live-code-'));
const links = [];
await writeFile(
  path.join(data, 'settings.json'),
  JSON.stringify({
    repoContext: false,
    verifyChanges: false,
    orchestra: false,
    closeToTray: false,
    codeMode: 'off',
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
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9226',
  },
});
let browser;
try {
  for (let i = 0; i < 100 && !browser; i++) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9226');
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
  for (const provider of providers) {
    const prefix = process.env['LUMEN_LIVE_ACCOUNT_' + provider.toUpperCase()] || '';
    const actual = accounts.find((a) => a.provider === provider && a.id.startsWith(prefix));
    assert(actual, provider + ' has no configured Lumen account');
    const a = await invoke('add_account', { name: 'Live ' + provider + ' Code Mode', provider });
    const isolated = path.join(data, 'accounts', a.id);
    for (const name of await readdir(isolated)) {
      assert.equal(name, 'config.toml');
      await unlink(path.join(isolated, name));
    }
    await rmdir(isolated);
    await symlink(path.join(original, 'accounts', actual.id), isolated, 'junction');
    links.push(isolated);
    assert.equal(
      await invoke('account_status', { id: a.id }),
      true,
      provider + ' is not signed in',
    );
    const models = await invoke('list_models', { id: a.id, refresh: true });
    const want = process.env['LUMEN_LIVE_MODEL_' + provider.toUpperCase()];
    const model =
      (want
        ? models.find((m) => m.id === want)
        : provider === 'claude'
          ? models.find((m) => /haiku/i.test(m.id + ' ' + m.name))
          : models.find((m) => m.isDefault)) || models[0];
    for (let seed = 0; seed < seeds; seed++)
      for (const [key, c] of Object.entries(cases)) {
        if (only && !only.includes(key)) continue;
        // Alternate arm order within each pair.
        for (const arm of (seed + key.charCodeAt(0)) % 2 ? ['CM', 'TC'] : ['TC', 'CM']) {
          const prefs = await invoke('get_settings');
          await invoke('save_settings', {
            value: { ...prefs, codeMode: arm === 'CM' ? 'on' : 'off' },
          });
          const workspace = path.join(data, [provider, key, seed, arm].join('-'));
          await mkdir(workspace);
          await c.prepare?.(workspace);
          const s = await invoke('create_session', { workspace });
          const started = Date.now();
          await invoke('send_message', {
            id: s.id,
            text: c.prompt,
            options: {
              mode: provider,
              accountId: a.id,
              model: model.id,
              effort: '',
              accessMode: 'autopilot',
              autoReview: false,
            },
          });
          let status;
          const answered = new Set();
          const questions = [];
          for (let i = 0; i < 1200; i++) {
            status = (await invoke('list_sessions', { search: '', offset: 0 })).find(
              (x) => x.id === s.id,
            )?.status;
            if (['completed', 'error', 'interrupted'].includes(status)) break;
            // Only the consent for this run's own project MCP fixture is answered.
            const open = (await invoke('get_messages', { id: s.id }))
              .at(-1)
              ?.activity.filter((x) => x.kind === 'interaction')
              .map((x) => JSON.parse(x.text))
              .find((q) => !answered.has(q.id));
            if (open) {
              answered.add(open.id);
              const text = JSON.stringify(open);
              questions.push(text.slice(0, 600));
              const allow = open.questions?.[0]?.options?.find((o) =>
                /^(accept|allow)/i.test(o.label),
              );
              if (key === 'D' && /run tool \\"(github|filesystem)_[a-z_]+\\"\?/.test(text) && allow)
                await invoke('answer_question', {
                  id: open.id,
                  answers: { [open.questions[0].id]: [allow.label] },
                });
            }
            await sleep(500);
          }
          if (!['completed', 'error', 'interrupted'].includes(status))
            await invoke('cancel_run', { id: s.id }).catch(() => {});
          const last = (await invoke('get_messages', { id: s.id })).at(-1);
          const activity = last?.activity || [];
          const answer = (last?.text || '').trim();
          const success =
            status === 'completed' &&
            (c.judge ? await c.judge(workspace) : answer.replace(/[`.*]/g, '').trim() === c.answer);
          const row = {
            tier: 'T3',
            provider,
            model: model.id,
            suite: key,
            seed,
            arm,
            status,
            success,
            wallMs: Date.now() - started,
            answer: answer.slice(0, 200),
            tools: activity.filter((x) => x.kind === 'tool').map((x) => x.text.slice(0, 80)),
            codeCalls: activity.filter((x) => x.kind === 'code-call').length,
            usage: activity.filter((x) => x.kind === 'usage').map((x) => x.text),
            execution: activity
              .filter((x) => x.kind === 'execution')
              .map((x) => x.text.slice(0, 4000)),
            kinds: [...new Set(activity.map((x) => x.kind))],
            questions,
            errors: activity
              .filter((x) => /error|connector-status/.test(x.kind))
              .map((x) => x.text.slice(0, 600)),
          };
          await appendFile(output, JSON.stringify(row) + '\n');
          console.log(
            provider,
            key,
            seed,
            arm,
            status,
            success,
            Math.round(row.wallMs / 1000) + 's',
          );
        }
      }
  }
} finally {
  await browser?.close().catch(() => {});
  await new Promise((r) => {
    const p = spawn(
      path.join(root, 'src-tauri/target/debug/lumen-daemon.exe'),
      ['--data-dir', data, '--shutdown'],
      { stdio: 'ignore', windowsHide: true },
    );
    p.on('exit', r);
    p.on('error', r);
  });
  child.kill();
  for (const link of links) await unlink(link).catch(() => rmdir(link));
  site.close();
}
