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
// TC classic, CM Code Mode always, AUTO Automatic (direct tools, large results held back).
const modes = { TC: 'off', CM: 'on', AUTO: 'auto' };
const arms = (process.env.LUMEN_BENCH_ARMS || 'TC,CM').split(',');
assert(
  arms.every((a) => modes[a]),
  'LUMEN_BENCH_ARMS takes TC, CM and AUTO',
);
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
const mcpServer = `import{createInterface}from'node:readline';const tools=${JSON.stringify(tools)};for await(const line of createInterface({input:process.stdin})){const v=JSON.parse(line);let result={};if(v.method==='initialize')result={protocolVersion:'2025-06-18',serverInfo:{name:'acme',version:'1'},capabilities:{tools:{}}};else if(v.method==='tools/list')result={tools};else if(v.method==='tools/call'){const n=v.params.name,a=v.params.arguments||{};const big=(p)=>{const m=String(p).match(/src\\/big-(\\d\\d)\\.ts$/);if(!m)return null;const filler=Array.from({length:600},(_,k)=>'// '+k+' lorem ipsum dolor sit amet consectetur').join('\\n');return filler+'\\n'+Array.from({length:Number(m[1])%4+1},(_,k)=>'export const v'+k+' = '+k+';').join('\\n')+'\\n'};const mod=(p)=>{const m=String(p).match(/src\\/mod-(\\d\\d)\\.ts$/);return m?Array.from({length:Number(m[1])%3+1},(_,k)=>'export function f'+k+'() {}').join('\\n')+'\\nconst internal = 1;\\n':null};if(n==='filesystem_read_multiple_files'&&Array.isArray(a.paths)){console.log(JSON.stringify({jsonrpc:'2.0',id:v.id,result:{content:[{type:'text',text:a.paths.map(p=>p+':\\n'+(mod(p)??'No such file')).join('\\n---\\n')}]}}));continue;}const text=n==='github_list_issues'&&a.repo==='web'?${JSON.stringify(issues)}:(n==='filesystem_read_file'||n==='github_get_file_contents')&&String(a.path).endsWith('src/date/format.ts')?${JSON.stringify(source)}:n==='filesystem_read_file'&&big(a.path)?big(a.path):n==='filesystem_read_file'&&/src\\/mod-(\\d\\d)\\.ts$/.test(String(a.path))?Array.from({length:Number(String(a.path).match(/(\\d\\d)\\.ts$/)[1])%3+1},(_,k)=>'export function f'+k+'() {}').join('\\n')+'\\nconst internal = 1;\\n':'No results';result={content:[{type:'text',text}]};}if(v.id!==undefined)console.log(JSON.stringify({jsonrpc:'2.0',id:v.id,result}));}`;

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
  // Probe, run only when selected: is lumen_code offered and callable at all?
  P: {
    probe: true,
    prompt:
      'Use the lumen_code tool to run a script that returns 6*7. Answer with the result only.',
    answer: '42',
  },
  G: {
    prompt:
      'With the acme filesystem tools, read src/mod-01.ts through src/mod-12.ts and answer with the total number of exported functions across the twelve files, as an integer alone.',
    prepare: (w) => cases.D.prepare(w),
    answer: String(Array.from({ length: 12 }, (_, i) => ((i + 1) % 3) + 1).reduce((a, b) => a + b)),
  },
  H: {
    prompt:
      'With the acme filesystem tools, read src/big-01.ts through src/big-12.ts and answer with the total number of exported constants across the twelve files, as an integer alone.',
    prepare: (w) => cases.D.prepare(w),
    answer: String(Array.from({ length: 12 }, (_, i) => ((i + 1) % 4) + 1).reduce((a, b) => a + b)),
  },
  D: {
    prompt:
      'In the GitHub repository acme/web, find the open issue labelled regression. Its body names a source file. Read that file with the acme filesystem tools and answer with the name of the function it exports, alone.',
    async prepare(w) {
      // The fixture server lives outside the workspace so its answers cannot be read as files.
      const server = path.join(path.dirname(w), 'fixtures', path.basename(w) + '-acme.mjs');
      await mkdir(path.dirname(server), { recursive: true });
      await writeFile(server, mcpServer);
      await writeFile(
        path.join(w, '.mcp.json'),
        JSON.stringify({ mcpServers: { acme: { command: process.execPath, args: [server] } } }),
      );
    },
    answer: 'formatInstant',
  },
};
// Cumulative usage from the CLI's own local logs (transcripts, never credentials).
async function walk(dir, out = []) {
  for (const e of await readdir(dir, { withFileTypes: true }).catch(() => [])) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) await walk(p, out);
    else if (e.name.endsWith('.jsonl')) out.push(p);
  }
  return out;
}
async function nativeUsage(provider, home, workspace, since) {
  const { stat } = await import('node:fs/promises');
  const files = await walk(path.join(home, provider === 'claude' ? 'projects' : 'sessions'));
  for (const f of files) {
    if ((await stat(f)).mtimeMs < since) continue;
    const text = await readFile(f, 'utf8');
    // Match on folder names: the temp root may be an 8.3 short path in our copy.
    if (
      !text.includes(path.basename(path.dirname(workspace))) ||
      !text.includes(path.basename(workspace))
    )
      continue;
    const lines = text
      .split('\n')
      .filter(Boolean)
      .map((l) => {
        try {
          return JSON.parse(l);
        } catch {
          return {};
        }
      });
    if (provider === 'claude') {
      const seen = new Map();
      for (const l of lines)
        if (l.type === 'assistant' && l.message?.usage) seen.set(l.message.id, l.message.usage);
      const u = [...seen.values()];
      const input = u.reduce(
        (a, x) =>
          a +
          (x.input_tokens || 0) +
          (x.cache_creation_input_tokens || 0) +
          (x.cache_read_input_tokens || 0),
        0,
      );
      return {
        requests: u.length,
        inputTokens: input,
        cachedTokens: u.reduce((a, x) => a + (x.cache_read_input_tokens || 0), 0),
        outputTokens: u.reduce((a, x) => a + (x.output_tokens || 0), 0),
      };
    }
    const counts = lines.filter((l) => l.payload?.type === 'token_count' && l.payload.info);
    const t = counts.at(-1)?.payload.info.total_token_usage;
    if (t)
      return {
        requests: counts.length,
        inputTokens: t.input_tokens,
        cachedTokens: t.cached_input_tokens,
        outputTokens: t.output_tokens,
      };
  }
  return null;
}
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
        if (only ? !only.includes(key) : c.probe) continue;
        // Alternate arm order within each pair.
        const pair = arms.slice();
        if ((seed + key.charCodeAt(0)) % 2) pair.reverse();
        for (const arm of pair) {
          const prefs = await invoke('get_settings');
          // codeModeChosen keeps an explicit Off from being read as the Automatic default.
          await invoke('save_settings', {
            value: { ...prefs, codeMode: modes[arm], codeModeChosen: true },
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
              if (
                ['D', 'G', 'H'].includes(key) &&
                /run tool \\"(acme_)?(github|filesystem)_[a-z_]+\\"\?/.test(text) &&
                allow
              )
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
            (c.judge
              ? await c.judge(workspace)
              : (answer
                  .replace(/[`*]/g, '')
                  .trim()
                  .match(/([A-Za-z0-9_]+)\.?$/)?.[1] ?? '') === c.answer);
          // Claude Code reports turn totals in its result event; older runs read transcripts.
          const reported = activity.find((x) => x.kind === 'native-usage')?.text;
          const native = reported
            ? JSON.parse(reported)
            : await nativeUsage(
                provider,
                path.join(original, 'accounts', actual.id),
                workspace,
                started - 1000,
              );
          const row = {
            native,
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
