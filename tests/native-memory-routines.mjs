// Project memory, plain-language routines, /every and recipes through the real GUI and daemon.
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';

const h = await nativeHarness('memory-routines');
try {
  const claude = await h.invoke('add_account', { name: 'Claude', provider: 'claude' });
  const options = {
    mode: 'claude',
    accountId: claude.id,
    model: 'sonnet',
    effort: 'low',
    accessMode: 'read-only',
  };
  const chat = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('send_message', {
    id: chat.id,
    text: 'FEATURE_TAGS controlla ogni 30 minuti di aggiornare le dipendenze fino al 30 agosto 2027',
    options,
  });
  let reply;
  for (let i = 0; i < 600 && !reply; i++) {
    const s = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
      (x) => x.id === chat.id,
    );
    if (['completed', 'error', 'interrupted'].includes(s.status))
      reply = (await h.invoke('get_messages', { id: chat.id })).at(-1);
    else await new Promise((r) => setTimeout(r, 100));
  }
  assert(reply, 'turn timed out');
  assert.equal(reply.text, 'Dependencies checked.', JSON.stringify(reply));
  const notes = await readFile(path.join(h.workspace, 'LUMEN.md'), 'utf8');
  assert.match(notes, /## Lumen memory\n- Use pnpm for installs _\(\d{4}-\d{2}-\d{2}\)_/);
  const [routine] = await h.invoke('list_routines', { id: chat.id });
  assert.equal(routine.text, "Update this project's dependencies");
  assert.equal(routine.everyMs, 30 * 60_000);
  assert.equal(new Date(routine.untilAt).toISOString(), '2027-08-30T23:59:59.000Z');
  // The rendered transcript hides the tags and shows both confirmations and the routine strip.
  await h.page.reload();
  await h.page.locator('main.workspace').waitFor();
  const logo = h.page.locator('.sidebar-toggle');
  if ((await logo.getAttribute('aria-expanded')) === 'false') await logo.click();
  await h.page
    .locator('#conversation-sidebar')
    .getByText(/FEATURE_TAGS/)
    .first()
    .click();
  await h.page.getByText('Dependencies checked.').first().waitFor();
  await h.page.screenshot({ path: path.join(h.data, 'memory-routines.png') });
  console.log(`Screenshot: ${path.join(h.data, 'memory-routines.png')}`);
  const body = await h.page.locator('body').innerText();
  assert(!body.includes('lumen-remember') && !body.includes('lumen-routine'));
  assert(/Remembered in LUMEN.md|Ricordato in LUMEN.md/.test(body), body.slice(0, 2000));
  assert(/Recurring task scheduled|Attività ricorrente programmata/.test(body));
  assert(/(Every|Ogni) 30m/.test(body));
  assert(
    body.includes(
      new Date(Date.UTC(2027, 7, 30)).toLocaleDateString(undefined, { timeZone: 'UTC' }),
    ),
  );
  // Explicit /every through the composer API and stop.
  const manual = await h.invoke('create_routine', {
    id: chat.id,
    text: 'check outdated packages',
    every: '1d',
    options,
  });
  assert.equal((await h.invoke('list_routines', { id: chat.id })).length, 2);
  await h.invoke('delete_routine', { routine: manual.id });
  assert.equal((await h.invoke('list_routines', { id: chat.id })).length, 1);
  const weekly = await h.invoke('create_routine', {
    id: chat.id,
    text: 'summarize open TODOs',
    every: 'lunedì',
    at: '9:00',
    options,
  });
  assert.equal(weekly.anchor, 'mon@09:00');
  const next = new Date(weekly.nextAt);
  assert.deepEqual([next.getDay(), next.getHours(), next.getMinutes()], [1, 9, 0]);
  await h.page.reload();
  await h.page
    .locator('#conversation-sidebar')
    .getByText(/FEATURE_TAGS/)
    .first()
    .click();
  const monday = new Date(Date.UTC(2024, 0, 1)).toLocaleDateString(undefined, {
    weekday: 'long',
    timeZone: 'UTC',
  });
  await h.page.getByText(`${monday} 09:00: summarize open TODOs`).waitFor();
  await h.invoke('delete_routine', { routine: weekly.id });
  // Recipe from the finished reply.
  const name = await h.invoke('save_recipe', { id: chat.id, message: reply.id });
  const skills = await readdir(path.join(h.data, 'shared', 'skills'));
  assert(skills.includes(name), `${name} not in ${skills}`);
  const recipe = await readFile(path.join(h.data, 'shared', 'skills', name, 'SKILL.md'), 'utf8');
  assert(recipe.includes('description: Saved recipe: FEATURE_TAGS controlla ogni 30 minuti'));
  console.log(
    'Passed: memory saved to LUMEN.md, plain-language routine with end date, tags hidden in GUI, /every create/stop, recipe saved as shared skill. No live inference.',
  );
} finally {
  await h.close();
}
