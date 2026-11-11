// Native authentication only. Read account metadata, never credential contents.
import { execFileSync } from 'node:child_process';
import { readdir, unlink, rmdir, symlink, lstat } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';

export function accountMetadata() {
  const root =
    process.env.LUMEN_LIVE_ACCOUNT_ROOT ||
    path.join(process.env.APPDATA, 'io.github.andrea17982.lumen');
  const text = execFileSync(
    'python',
    [
      '-c',
      'import sqlite3,sys,json; c=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True); print(json.dumps(c.execute("SELECT provider,id FROM accounts WHERE provider IN (\'claude\',\'gpt\')").fetchall()))',
      path.join(root, 'lumen.db'),
    ],
    { encoding: 'utf8', windowsHide: true },
  );
  return { root, accounts: JSON.parse(text).map(([provider, id]) => ({ provider, id })) };
}
export async function attachLiveAccounts(h, providers = ['claude', 'gpt']) {
  const metadata = accountMetadata();
  const links = [];
  const accounts = [];
  for (const provider of providers) {
    const actual = metadata.accounts.find((a) => a.provider === provider);
    if (!actual) {
      accounts.push({ provider, ready: false, reason: 'No configured account' });
      continue;
    }
    assert(/^[a-f0-9-]{36}$/i.test(actual.id));
    const account = await h.invoke('add_account', { name: `Quality ${provider}`, provider });
    const isolated = path.join(h.data, 'accounts', account.id);
    const contents = await readdir(isolated);
    assert(contents.every((name) => name === 'config.toml'));
    for (const name of contents) await unlink(path.join(isolated, name));
    await rmdir(isolated);
    const original = path.join(metadata.root, 'accounts', actual.id);
    await symlink(original, isolated, 'junction');
    links.push(isolated);
    try {
      const signed = await h.invoke('account_status', { id: account.id });
      if (!signed) {
        accounts.push({ provider, ready: false, reason: 'Native CLI not signed in' });
        continue;
      }
      const models = await h.invoke('list_models', { id: account.id, refresh: true });
      const model =
        models.find((m) =>
          provider === 'claude'
            ? /sonnet/i.test(m.id + ' ' + m.name)
            : /terra/i.test(m.id + ' ' + m.name),
        ) ||
        models.find((m) => m.isDefault) ||
        models[0];
      assert(model);
      accounts.push({
        provider,
        ready: true,
        account,
        original,
        model,
        effort: model.efforts.includes('low') ? 'low' : model.defaultEffort || '',
      });
    } catch {
      accounts.push({
        provider,
        ready: false,
        reason: 'Native authentication/catalog preflight failed',
      });
    }
  }
  return {
    accounts,
    async cleanup() {
      // Never recursively delete a junction, nor an original account home.
      for (const link of links) {
        assert(path.resolve(link).startsWith(path.resolve(h.data) + path.sep));
        assert((await lstat(link)).isSymbolicLink());
        await unlink(link).catch(() => rmdir(link));
      }
    },
  };
}
