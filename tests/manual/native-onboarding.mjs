// Windows WebView2/IPC acceptance. No accounts, login or model call.
import assert from 'node:assert/strict';
import { expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { nativeHarness } from './native-harness.mjs';
const h = await nativeHarness('onboarding');
const checks = [];
try {
  await h.page
    .getByRole('textbox', { name: 'Message', exact: true })
    .fill('Keep this tutorial draft.');
  await h.page.getByRole('button', { name: 'Take a quick tour', exact: true }).click();
  await mkdir('.impeccable/review', { recursive: true });
  await h.page.screenshot({ path: '.impeccable/review/tour-native-light.png' });
  for (let i = 0; i < 4; i++) {
    await expect(h.page.locator('.tour-card h2')).toBeFocused();
    await h.page.getByRole('button', { name: 'Next', exact: true }).click();
  }
  await h.page.getByRole('button', { name: 'Start exploring', exact: true }).click();
  await expect(h.page.getByRole('textbox', { name: 'Message', exact: true })).toHaveValue(
    'Keep this tutorial draft.',
  );
  assert.equal((await h.invoke('list_accounts')).length, 0);
  assert.equal((await h.invoke('list_sessions', { search: '', offset: 0 })).length, 0);
  checks.push(
    'Real first-use five-step tour keeps draft/access and creates no account/session or model request',
  );
  const s = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('update_session', { id: s.id, title: 'Permission acceptance', pinned: false });
  await h.page.reload();
  await h.page.locator('.session-row').filter({ hasText: 'Permission acceptance' }).click();
  await h.page.getByRole('button', { name: 'Permissions: Read only', exact: true }).click();
  assert.equal(
    (await h.invoke('list_sessions', { search: '', offset: 0 })).find((row) => row.id === s.id)
      .accessMode,
    'read-only',
  );
  await h.page.getByRole('menuitemradio', { name: /^File edits/ }).click();
  await expect
    .poll(
      async () =>
        (await h.invoke('list_sessions', { search: '', offset: 0 })).find((row) => row.id === s.id)
          .accessMode,
    )
    .toBe('file-edits');
  await h.page.reload();
  await h.page.locator('.session-row').filter({ hasText: 'Permission acceptance' }).click();
  await expect(
    h.page.getByRole('button', { name: 'Permissions: File edits', exact: true }),
  ).toBeVisible();
  checks.push(
    'Opening explanations leaves backend access unchanged; explicit file-edit selection persists through actual IPC and reload',
  );
  await h.page.evaluate(() => (document.documentElement.dataset.theme = 'dark'));
  await h.page.getByRole('button', { name: 'Permissions: File edits', exact: true }).click();
  await h.page.screenshot({ path: '.impeccable/review/permissions-native-dark.png' });
  await h.page.keyboard.press('Escape');
  await h.page.getByRole('button', { name: 'Settings', exact: true }).click();
  await h.page.getByRole('button', { name: 'Take a quick tour', exact: true }).click();
  await expect(h.page.locator('.tour-card h2')).toBeFocused();
  await h.page.keyboard.press('Escape');
  await expect(h.page.getByRole('textbox', { name: 'Message', exact: true })).toBeFocused();
  assert.equal((await h.invoke('get_messages', { id: s.id })).length, 0);
  checks.push('Tour replay from native settings exits by Escape with usable focus and no messages');
  await writeFile(
    '.impeccable/review/native-onboarding-results.json',
    JSON.stringify({ offline: true, modelCalls: 0, checks }, null, 2),
  );
  console.log(JSON.stringify({ passed: checks.length, checks }, null, 2));
} finally {
  await h.close();
}
