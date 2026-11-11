import { execFileSync } from 'node:child_process';
import { readdir, readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { tmpdir } from 'node:os';

export function appendValidation(h, sessionId, label, status, detail) {
  // Only the harness-owned database is written, never the configured real app database.
  const relative = path.relative(path.resolve(tmpdir()), path.resolve(h.data));
  assert(!relative.startsWith('..') && !path.isAbsolute(relative));
  assert(/^lumen-[^\\/]+$/.test(relative));
  const activity = {
    kind: 'validation',
    text: JSON.stringify({
      label,
      status,
      detail,
      diagnostics: [],
      workspace: h.validationWorkspace,
    }),
  };
  const script =
    'import sqlite3,sys,json; c=sqlite3.connect(sys.argv[1]); row=c.execute("SELECT id,activity FROM messages WHERE session_id=? AND role=\'assistant\' ORDER BY created_at DESC LIMIT 1",(sys.argv[2],)).fetchone(); assert row; rows=json.loads(row[1]); rows.append(json.loads(sys.argv[3])); c.execute("UPDATE messages SET activity=? WHERE id=?",(json.dumps(rows),row[0])); c.commit()';
  execFileSync(
    'python',
    ['-c', script, path.join(h.data, 'lumen.db'), sessionId, JSON.stringify(activity)],
    { windowsHide: true, encoding: 'utf8' },
  );
}

async function findSessionFiles(directory, ids, depth = 0) {
  if (depth > 5) return [];
  const found = [];
  for (const entry of await readdir(directory, { withFileTypes: true }).catch(() => [])) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) found.push(...(await findSessionFiles(file, ids, depth + 1)));
    else if (entry.name.endsWith('.jsonl') && [...ids].some((id) => entry.name.includes(id)))
      found.push(file);
  }
  return found;
}
/** Only the exact native sessions created by this run are read; no private historical log is opened. */
export async function ownSessionUsage(account, ids) {
  const unique = new Set(ids.filter((id) => /^[a-f0-9-]{30,40}$/i.test(id)));
  const files = await findSessionFiles(
    path.join(account.original, account.provider === 'claude' ? 'projects' : 'sessions'),
    unique,
  );
  let input = 0,
    output = 0,
    cacheRead = 0,
    cacheWrite = 0,
    found = 0;
  for (const file of files) {
    if ((await stat(file)).size > 16 * 1024 * 1024) continue;
    const messages = new Map();
    let total;
    for (const line of (await readFile(file, 'utf8')).split('\n')) {
      let row;
      try {
        row = JSON.parse(line);
      } catch {
        continue;
      }
      if (
        account.provider === 'claude' &&
        row.type === 'assistant' &&
        typeof row.message?.id === 'string' &&
        row.message.usage
      )
        messages.set(row.message.id, row.message.usage);
      if (
        account.provider === 'gpt' &&
        row.type === 'event_msg' &&
        row.payload?.type === 'token_count' &&
        row.payload.info?.total_token_usage
      )
        total = row.payload.info.total_token_usage;
    }
    const rows = account.provider === 'claude' ? [...messages.values()] : total ? [total] : [];
    for (const usage of rows) {
      if (!Number.isFinite(usage.input_tokens) || !Number.isFinite(usage.output_tokens)) continue;
      input += usage.input_tokens;
      output += usage.output_tokens;
      cacheRead += usage.cache_read_input_tokens ?? usage.cached_input_tokens ?? 0;
      cacheWrite += usage.cache_creation_input_tokens ?? 0;
      found++;
    }
  }
  return found
    ? {
        source: 'exact owned native session logs',
        inputTokens: input,
        outputTokens: output,
        cacheReadTokens: cacheRead,
        cacheWriteTokens: cacheWrite,
        costUsd: null,
        costNote: 'No verified billed amount available; no price inferred',
        sessions: files.length,
      }
    : { source: 'unavailable', inputTokens: null, outputTokens: null, costUsd: null };
}
