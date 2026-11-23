// Explicitly authorized, bounded real-account acceptance. No credentials are read or copied.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile, unlink, mkdtemp, copyFile } from 'node:fs/promises';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { nativeHarness } from './native-harness.mjs';
import { accountMetadata, attachLiveAccounts } from './quality-live-accounts.mjs';

assert.equal(process.env.LUMEN_LIVE_ROUTING, '1', 'Enable only for an authorized live test');
const controlled = process.env.LUMEN_CONTROLLED_QUOTA === '1';
const output = path.resolve(
  `.codex/verification/live-routing-${controlled ? 'controlled-quota' : 'current'}.json`,
);
if (controlled) {
  const cli = await mkdtemp(path.join(tmpdir(), 'lumen-quota-cli-'));
  for (const file of ['claude.cmd', 'codex.cmd', 'catalog.ps1', 'schema.ps1', 'provider.ps1'])
    await copyFile(path.resolve('tests/fixtures/cli', file), path.join(cli, file));
  const schema = path.join(cli, 'schema.ps1');
  await writeFile(
    schema,
    (await readFile(schema, 'utf8')).replace(
      "Join-Path $PSScriptRoot '../../../src/features/gatekeeper/fixtures'",
      ` '${path.resolve('src/features/gatekeeper/fixtures').replaceAll("'", "''")}'`,
    ),
  );
  const catalog = path.join(cli, 'catalog.ps1');
  await writeFile(
    catalog,
    (await readFile(catalog, 'utf8')).replace(
      "if ($Provider -eq 'gpt') {",
      "if ($Provider -eq 'gpt') {\n        if ($request.method -in @('skills/list','mcpServerStatus/list')) { Emit @{id=$request.id;result=@{data=@();nextCursor=$null}}; continue }",
    ),
  );
  process.env.PATH = `${cli};${process.env.PATH}`;
}
const report = {
  startedAt: new Date().toISOString(),
  scope:
    'Real configured native accounts; isolated Lumen DB and disposable workspace; no credential copying',
  checks: [],
  controlledQuota: controlled,
  failureSource: controlled
    ? 'Offline Claude/GPT quota replies at CLI boundary; real Antigravity account and inference'
    : 'Real providers',
  configsRestored: false,
};
await mkdir(path.dirname(output), { recursive: true });
const configs = [];
const metadata = accountMetadata();
for (const account of metadata.accounts) {
  assert(/^[a-f0-9-]{36}$/i.test(account.id));
  for (const name of [
    'config.toml',
    'settings.json',
    '.gemini/config/mcp_config.json',
    '.gemini/settings.json',
  ]) {
    const file = path.join(metadata.root, 'accounts', account.id, name);
    try {
      configs.push({ file, bytes: await readFile(file) });
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
      configs.push({ file, bytes: null });
    }
  }
}
let h, homes;
const flush = () => writeFile(output, JSON.stringify(report, null, 2));
try {
  h = await nativeHarness('real-routing', { offline: false, appEnv: { NO_BROWSER: 'true' } });
  await h.invoke('save_settings', {
    value: {
      ...(await h.invoke('get_settings')),
      repoContext: false,
      verifyChanges: false,
      orchestra: false,
      closeToTray: false,
    },
  });
  homes = await attachLiveAccounts(h, ['claude', 'gpt', 'antigravity']);
  report.accounts = homes.accounts.map((a) => ({
    provider: a.provider,
    ready: a.ready,
    reason: a.reason,
  }));
  await flush();
  async function turn(label, text, options, existing) {
    const session = existing || (await h.invoke('create_session', { workspace: h.workspace }));
    const started = Date.now();
    const row = { label, requestedMode: options.mode, status: 'starting', switches: 0 };
    report.checks.push(row);
    await flush();
    try {
      const plan = await h.invoke('send_message', { id: session.id, text, options });
      row.selectedProvider = plan.provider;
      row.model = plan.model;
      const answered = new Set();
      while (Date.now() - started < 150000) {
        const messages = await h.invoke('get_messages', { id: session.id });
        const pending = await h.invoke('pending_questions');
        for (const item of messages.at(-1)?.activity ?? []) {
          if (item.kind !== 'interaction') continue;
          const question = JSON.parse(item.text);
          if (!pending.includes(question.id) || answered.has(question.id)) continue;
          assert(
            question.questions.every((q) => q.id === 'switch'),
            'Unexpected question: do not approve automatically',
          );
          await h.invoke('answer_question', { id: question.id, answers: { switch: ['Continue'] } });
          answered.add(question.id);
          row.switches++;
        }
        const state = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
          (s) => s.id === session.id,
        );
        if (['completed', 'error', 'interrupted'].includes(state.status)) {
          const message = (await h.invoke('get_messages', { id: session.id })).at(-1);
          row.status = state.status;
          row.elapsedMs = Date.now() - started;
          row.models = message.activity.filter((a) => a.kind === 'model').map((a) => a.text);
          row.errors = message.activity.filter((a) => a.kind === 'error').map((a) => a.text);
          row.fallbacks = message.activity
            .filter((a) => a.kind === 'fallback')
            .map((a) => a.text.replace(/\([^)]*\)/g, '(account)'));
          row.reply = message.text.slice(0, 500);
          await flush();
          return { row, session, message };
        }
        await new Promise((r) => setTimeout(r, 250));
      }
      await h.invoke('cancel_run', { id: session.id });
      row.status = 'timeout';
      row.elapsedMs = Date.now() - started;
    } catch (error) {
      row.status = 'failed';
      row.error = String(error);
    }
    await flush();
    return { row, session };
  }
  const google = homes.accounts.find((a) => a.provider === 'antigravity' && a.ready);
  if (google && !controlled) {
    const models = await h.invoke('list_models', { id: google.account.id });
    const model = models.find((m) => /flash/i.test(m.id)) || google.model;
    report.googleCatalogOnly = models.every((m) => m.id.startsWith('gemini-'));
    const options = {
      mode: 'antigravity',
      accountId: google.account.id,
      model: model.id,
      effort: model.efforts.includes('low') ? 'low' : '',
      accessMode: 'read-only',
      autoReview: false,
    };
    await writeFile(path.join(h.workspace, 'live-probe.txt'), 'LUMEN_REAL_FILE_A_7319');
    const first = await turn(
      'Antigravity reads actual file',
      'Read live-probe.txt in the current folder. Reply only with its exact content. Do not change files, use a browser or delegate.',
      options,
    );
    first.row.passed =
      first.row.status === 'completed' && first.row.reply.includes('LUMEN_REAL_FILE_A_7319');
    const second = await turn(
      'Antigravity consecutive turn is fresh',
      'Reply only LUMEN_REAL_REPLY_B_7319. Do not use tools or delegate.',
      options,
      first.session,
    );
    second.row.passed =
      second.row.status === 'completed' &&
      second.row.reply.includes('LUMEN_REAL_REPLY_B_7319') &&
      !second.row.reply.includes('LUMEN_REAL_FILE_A_7319');
    const explicit = await turn(
      'Auto respects explicit Antigravity',
      'Usa antigravity per questo prompt: reply only LUMEN_REAL_EXPLICIT_7319. Do not use tools or delegate.',
      { mode: 'auto', effort: 'low', accessMode: 'read-only', autoReview: false },
    );
    explicit.row.passed =
      explicit.row.selectedProvider === 'antigravity' &&
      explicit.row.status === 'completed' &&
      explicit.row.reply.includes('LUMEN_REAL_EXPLICIT_7319');
  }
  const auto = await turn(
    controlled ? 'Controlled quota chain to real Antigravity' : 'Auto real-account response',
    `${controlled ? 'QUOTA_CHAIN: ' : ''}Reply only LUMEN_REAL_FALLBACK_7319. Do not use tools, browser or subagents.`,
    {
      mode: controlled ? 'claude' : 'auto',
      ...(controlled
        ? {
            accountId: homes.accounts.find((a) => a.provider === 'claude').account.id,
            model: 'sonnet',
          }
        : {}),
      effort: 'low',
      accessMode: 'read-only',
      autoReview: false,
      balanceUsage: false,
    },
  );
  auto.row.passed =
    auto.row.status === 'completed' && auto.row.reply.includes('LUMEN_REAL_FALLBACK_7319');
  report.realFallbackObserved = !controlled && auto.row.switches > 0 && auto.row.passed;
  if (controlled) {
    assert.equal(auto.row.status, 'completed');
    assert(auto.row.passed);
    assert.equal(auto.row.switches, 2);
    assert(auto.row.models.some((m) => /gemini/i.test(m)));
    report.controlledFallbackToRealAccountPassed = true;
  }
  report.finishedAt = new Date().toISOString();
} finally {
  await h?.close();
  // Restore only non-credential configuration; unlink junctions before any later temp cleanup.
  for (const { file, bytes } of configs) {
    if (bytes) {
      await mkdir(path.dirname(file), { recursive: true });
      await writeFile(file, bytes);
    } else
      await unlink(file).catch((e) => {
        if (e.code !== 'ENOENT') throw e;
      });
  }
  await homes?.cleanup();
  report.configsRestored = true;
  await flush();
}
assert(report.checks.length > 0 && report.checks.every((row) => row.passed));
console.log(JSON.stringify(report, null, 2));
