// Windows acceptance with isolated data, real WebView2/IPC and offline CLI fixtures.
import assert from 'node:assert/strict';
import { expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';

const editorRoot = await mkdtemp(path.join(tmpdir(), 'lumen-editor-a11y-'));
const code = path.join(process.env.LOCALAPPDATA, 'Programs/Microsoft VS Code/bin/code.cmd');
const checks = [];
let editorAvailable = false;
try {
  await access(code);
  editorAvailable = true;
} catch {
  /* explicit prerequisite below */
}
if (editorAvailable)
  await writeFile(
    path.join(editorRoot, 'code.cmd'),
    `@echo off\r\ncall "${code}" --user-data-dir "${path.join(editorRoot, 'profile')}" --extensions-dir "${path.join(editorRoot, 'extensions')}" --new-window %*\r\n`,
  );
const originalPath = process.env.PATH;
process.env.PATH = `${editorRoot};${originalPath}`;
const h = await nativeHarness('accessibility');
process.env.PATH = originalPath;
const { page, invoke } = h;
const output = path.join(h.root, '.impeccable/accessibility');
await mkdir(output, { recursive: true });
try {
  const input = page.getByRole('textbox', { name: 'Message', exact: true });
  await input.fill('Keep this accessibility draft.');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const panel = page.getByRole('dialog', { name: 'Settings panel' });
  await expect(panel).toHaveAttribute('aria-modal', 'true');
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toHaveCount(0);
  await page.screenshot({ path: path.join(output, 'settings-light.png') });
  await panel.getByRole('button', { name: 'Dark', exact: true }).click();
  await page.screenshot({ path: path.join(output, 'settings-dark.png') });
  await panel.getByRole('button', { name: 'Close settings' }).click();
  await expect(input).toHaveValue('Keep this accessibility draft.');
  await expect(page.getByRole('button', { name: 'Settings', exact: true })).toBeFocused();
  checks.push(
    'Native settings isolates background controls, preserves a draft and restores focus in both themes',
  );

  const account = await invoke('add_account', {
    name: 'Offline accessibility',
    provider: 'claude',
  });
  const chat = await invoke('create_session', { workspace: h.workspace });
  await invoke('update_session', { id: chat.id, title: 'Accessibility question', pinned: false });
  await page.route('**/src/lib/bridge.ts*', async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const original = 'if (desktop) return invoke(command, args);';
    assert(source.includes(original));
    await route.fulfill({
      response,
      body: source.replace(
        original,
        'if (desktop) return window.__a11yQuestionIntercept ? window.__a11yQuestionIntercept(command, args, () => invoke(command, args)) : invoke(command, args);',
      ),
    });
  });
  await page.reload();
  await page.locator('.session-row').filter({ hasText: 'Accessibility question' }).click();
  await page.evaluate(() => {
    window.__a11yQuestionFault = true;
    window.__a11yAnswerFault = true;
    window.__a11yQuestionIntercept = (command, args, proceed) => {
      if (command === 'answer_question' && window.__a11yAnswerFault) {
        window.__a11yAnswerFault = false;
        return Promise.reject(new Error('Controlled answer delivery failure'));
      }
      return command === 'pending_questions' && window.__a11yQuestionFault
        ? new Promise((_, reject) =>
            setTimeout(() => reject(new Error('isolated read failure')), 800),
          )
        : proceed();
    };
  });
  await invoke('send_message', {
    id: chat.id,
    text: 'QUESTION_FIXTURE',
    options: {
      mode: 'claude',
      accountId: account.id,
      model: 'sonnet',
      effort: 'low',
      accessMode: 'autopilot',
    },
  });
  const questions = page.getByRole('form', { name: 'Questions from the agent' });
  await expect(questions.getByRole('status')).toHaveText('Checking question status…');
  await expect(questions.getByRole('status')).toHaveText('Question status unavailable. Retrying…');
  await page.evaluate(() => {
    window.__a11yQuestionFault = false;
  });
  const radio = questions.getByRole('radio', { name: /Native/ });
  await expect(radio).toBeEnabled();
  await expect(radio).not.toBeChecked();
  await radio.focus();
  await radio.press('Space');
  await questions.getByRole('button', { name: 'Send answers' }).focus();
  await page.keyboard.press('Enter');
  await expect(questions.getByRole('alert')).toHaveText('Controlled answer delivery failure');
  await expect(questions.getByRole('button', { name: 'Send answers' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(questions.getByRole('status')).toHaveText('Answers sent');
  await expect(questions.getByRole('status')).toBeFocused();
  await expect
    .poll(
      async () =>
        (await invoke('list_sessions', { search: '', offset: 0 })).find((s) => s.id === chat.id)
          ?.status,
    )
    .toBe('completed');
  checks.push(
    'Native question distinguishes loading/read failure, selects nothing automatically, accepts keyboard answer and restores status focus',
  );

  await page.getByRole('button', { name: 'Project inspector', exact: true }).click();
  const inspector = page.getByRole('complementary', { name: 'Project inspector' });
  await expect(inspector.getByRole('button', { name: 'Close inspector' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: 'Project inspector', exact: true })).toBeFocused();
  checks.push('Inspector keyboard dismissal returns focus to its opener');

  const longChat = await invoke('create_session', { workspace: h.workspace });
  await invoke('update_session', {
    id: longChat.id,
    title: 'Accessible long transcript',
    pinned: false,
  });
  const png =
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=';
  await writeFile(path.join(h.workspace, 'a11y.png'), Buffer.from(png, 'base64'));
  execFileSync(
    'python',
    [
      '-c',
      "import sqlite3,sys,json,uuid; c=sqlite3.connect(sys.argv[1]); rows=[(str(uuid.uuid4()),sys.argv[2],'assistant','A11Y HISTORICAL '+str(i)+' — à ⚙'+ ('\\n\\n![Local accessibility image](a11y.png)' if i==999 else ''),'[]',1000+i) for i in range(1000)]; c.executemany('INSERT INTO messages(id,session_id,role,text,activity,created_at) VALUES(?,?,?,?,?,?)',rows); c.commit()",
      path.join(h.data, 'lumen.db'),
      longChat.id,
    ],
    { windowsHide: true },
  );
  await page.reload();
  await page.locator('.session-row').filter({ hasText: 'Accessible long transcript' }).click();
  await expect(page.locator('.message')).toHaveCount(1000);
  const cdp = await page.context().newCDPSession(page);
  const ax = await cdp.send('Accessibility.getFullAXTree');
  const names = ax.nodes
    .filter((node) => !node.ignored)
    .map((node) => node.name?.value || '')
    .join('\n');
  assert(names.includes('A11Y HISTORICAL 0') && names.includes('A11Y HISTORICAL 999'));
  assert(await page.evaluate(() => window.find('A11Y HISTORICAL 1')));
  const enlarge = page.getByRole('button', { name: 'Enlarge image: Local accessibility image' });
  await enlarge.click();
  await expect(page.getByRole('dialog', { name: 'Local accessibility image' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(enlarge).toBeFocused();
  checks.push(
    '1000 native messages retain historical find/AX content and local image dialog restores focus',
  );
  await cdp.detach();

  const source = path.join(h.workspace, 'editor accessibility.ts');
  await writeFile(
    source,
    Array.from({ length: 24 }, (_, i) => `// Accessibility line ${i + 1}`).join('\n'),
  );
  if (editorAvailable && process.env.LUMEN_A11Y_HOLD === '1') {
    assert.equal(
      await invoke('open_source_location', {
        folder: h.workspace,
        file: path.basename(source),
        line: 13,
      }),
      true,
    );
    checks.push(
      'Installed VS Code accepted native source-location launch at line 13 with an isolated profile; visual line check remains separate',
    );
  }
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(
      {
        passed: true,
        checks,
        editorAvailable,
        editorRoot,
        pid: h.pid,
        scope:
          'Windows debug WebView2, real IPC/SQLite and offline CLI; CDP AX is not a screen-reader speech certification.',
      },
      null,
      2,
    ),
  );
  process.stdout.write(JSON.stringify({ passed: true, checks, pid: h.pid, editorRoot }) + '\n');
  if (process.env.LUMEN_A11Y_HOLD === '1') {
    const release = path.join(output, `release-fixture-${h.pid}`);
    while (true) {
      try {
        await access(release);
        break;
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 250));
      }
    }
  }
} finally {
  await h.close();
}
