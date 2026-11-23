// Real Windows WebView2/IPC plus offline app-server frames; no sign-in or inference.
import assert from 'node:assert/strict';
import { expect } from '@playwright/test';
import { nativeHarness } from './native-harness.mjs';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';

const h = await nativeHarness('connectors');
const { page, invoke, workspace } = h;
const checks = [];
const captures = path.join(h.root, '.impeccable/review');
await mkdir(captures, { recursive: true });
try {
  const account = await invoke('add_account', { name: 'Offline GPT', provider: 'gpt' });
  async function start(kind, selected = account, accessMode = 'autopilot') {
    const session = await invoke('create_session', { workspace });
    await invoke('update_session', { id: session.id, title: kind, pinned: false });
    await invoke('send_message', {
      id: session.id,
      text: `CONNECTOR_FIXTURE ${kind}`,
      options: {
        mode: selected.provider,
        accountId: selected.id,
        model: selected.provider === 'gpt' ? 'gpt-fixture' : 'sonnet',
        effort: 'low',
        accessMode,
      },
    });
    await page.reload();
    await page.locator('.session-row').filter({ hasText: kind }).click();
    return session;
  }
  const form = page.getByRole('form', { name: 'Request from a connector' });
  async function done(session) {
    await expect
      .poll(
        async () =>
          (await invoke('list_sessions', { search: '', offset: 0 })).find(
            (s) => s.id === session.id,
          )?.status,
        { timeout: 20000 },
      )
      .toBe('completed');
    const messages = await invoke('get_messages', { id: session.id });
    assert(messages.at(-1).text.includes('CONNECTOR_WIRE_OK'));
    assert(!JSON.stringify(messages).includes('never-store-this'));
    return messages;
  }
  const accepted = await start('FORM');
  await expect(form).toBeVisible({ timeout: 20000 });
  await expect(form).toHaveAttribute('data-pending', 'true');
  await expect.poll(async () => (await form.boundingBox())?.y).toBeGreaterThanOrEqual(0);
  await page.screenshot({ path: path.join(captures, 'connector-form.png') });
  for (const radio of await form.getByRole('radio').all()) await expect(radio).not.toBeChecked();
  await expect(form.getByRole('button', { name: 'Send answers' })).toBeDisabled();
  await form.getByRole('radio', { name: 'Accept', exact: true }).check();
  await form.getByRole('textbox', { name: 'Your answer', exact: true }).first().fill('bad');
  await form.getByRole('button', { name: 'Send answers' }).click();
  await expect(form.getByRole('alert')).toContainText('Invalid value');
  await form.getByRole('textbox', { name: 'Your answer', exact: true }).first().fill('3');
  await form.getByRole('button', { name: 'Send answers' }).click();
  const rows = (await done(accepted)).flatMap((m) => m.activity);
  assert.deepEqual(JSON.parse(rows.find((a) => a.kind === 'interaction-result').text).answers, {});
  checks.push(
    'Autopilot waits for explicit input; invalid types retry, optional field stays empty, typed response reaches CLI and answers are redacted',
  );
  const declined = await start('DECLINE');
  await expect(form).toBeVisible({ timeout: 20000 });
  await form.getByRole('radio', { name: 'Decline', exact: true }).check();
  await form.getByRole('button', { name: 'Send answers' }).click();
  await done(declined);
  checks.push('Decline sends native null content without filling required fields');
  const url = await start('URL');
  await expect(form.getByRole('button', { name: 'Open sign-in page' })).toBeVisible({
    timeout: 20000,
  });
  await expect(form).toContainText('https://example.com');
  await expect(form).not.toContainText('never-store-this');
  await page.screenshot({ path: path.join(captures, 'connector-url.png') });
  await form.getByRole('radio', { name: 'Accept', exact: true }).check();
  await form.getByRole('button', { name: 'Send answers' }).click();
  await done(url);
  checks.push(
    'URL request reveals origin, requires a choice, never auto-opens browser or persists signed URL',
  );
  const expired = await start('URL EXPIRE');
  await done(expired);
  const interaction = (await invoke('get_messages', { id: expired.id }))
    .flatMap((m) => m.activity)
    .find((a) => a.kind === 'interaction');
  await assert.rejects(invoke('open_connector_url', { id: JSON.parse(interaction.text).id }));
  checks.push('Resolved request expires consent and cannot reopen its URL');
  await done(await start('UNSUPPORTED'));
  checks.push('Unsupported schema receives native cancel and completes without waiting');
  const stopped = await start('URL STOP');
  await expect(form).toBeVisible({ timeout: 20000 });
  const active = await invoke('pending_questions');
  assert.equal(active.length, 1);
  await page.getByRole('button', { name: 'Stop response', exact: true }).click();
  await expect
    .poll(
      async () =>
        (await invoke('list_sessions', { search: '', offset: 0 })).find((s) => s.id === stopped.id)
          ?.status,
    )
    .toBe('interrupted');
  assert.deepEqual(await invoke('pending_questions'), []);
  await assert.rejects(invoke('open_connector_url', { id: active[0] }));
  checks.push('Stop expires pending connector consent and signed URL without replay');
  const claude = await invoke('add_account', { name: 'Offline Claude', provider: 'claude' });
  const reviewed = await start('CLAUDE_MCP_FIXTURE', claude, 'ask');
  await expect(page.getByRole('button', { name: 'Approve once', exact: true })).toBeEnabled({
    timeout: 20000,
  });
  await page.getByRole('button', { name: 'Approve once', exact: true }).click();
  const claudeRows = (await done(reviewed)).flatMap((m) => m.activity);
  assert(claudeRows.some((a) => a.kind === 'connector-status' && a.text.includes('connected')));
  checks.push(
    'Claude MCP waits for Ask approval, preserves exact input and shows sanitized connection status',
  );
  const elicited = await start('CLAUDE_ELICIT_FIXTURE', claude);
  await expect(form).toBeVisible({ timeout: 20000 });
  await form.getByRole('radio', { name: 'Accept', exact: true }).check();
  await form.getByRole('textbox', { name: 'Your answer', exact: true }).fill('3');
  await form.getByRole('button', { name: 'Send answers' }).click();
  await done(elicited);
  checks.push(
    'Claude native elicitation gets typed explicit consent in Autopilot without storing answers',
  );
  console.log(JSON.stringify({ checks, liveCalls: 0 }, null, 2));
} finally {
  await h.close();
}
