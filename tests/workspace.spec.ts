import { expect, test } from '@playwright/test';

test('every connected provider can be selected directly without changing its model family', async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: ['claude', 'gpt', 'gemini', 'antigravity', 'ollama'].map((provider) => ({
          id: provider,
          name: provider,
          provider,
        })),
        sessions: [],
      }),
    ),
  );
  await page.goto('/');
  const picker = page.getByRole('group', { name: 'AI provider' });
  for (const name of ['Gemini CLI', 'Antigravity', 'Ollama', 'Claude', 'GPT']) {
    const button = picker.getByRole('button', { name, exact: true });
    await button.click();
    await expect(button).toHaveAttribute('aria-pressed', 'true');
  }
});

test('slash commands can be filtered, selected and dismissed in the composer', async ({ page }) => {
  await page.goto('/');
  const input = page.getByRole('textbox', { name: 'Message', exact: true });
  await input.fill('/');
  const menu = page.getByRole('listbox', { name: 'Available commands' });
  await expect(menu.getByRole('option')).toHaveCount(2);
  await input.press('Enter');
  await expect(input).toHaveValue('/workspace ');
  await expect(menu).toBeHidden();
  await input.press('Enter');
  await expect(page.getByRole('alert')).toContainText('Enter an absolute path after /workspace.');
  await input.fill('/comp');
  await expect(menu.getByRole('option')).toHaveCount(1);
  await input.press('Escape');
  await expect(menu).toBeHidden();
  await input.fill('/compact');
  await input.press('Tab');
  await expect(input).toHaveValue('/compact');
  await expect(menu).toBeHidden();
  await input.press('Enter');
  await expect(page.getByRole('alert')).toContainText('Open a conversation with messages');
  await expect(input).toHaveValue('/compact');
  await input.fill('/comp');
  await menu.getByRole('option', { name: /compact/i }).click();
  await expect(input).toHaveValue('/compact');
  await expect(menu).toBeHidden();
});

test('usage shows an empty state when no cloud account is configured', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Provider usage' })).toContainText(
    'No cloud providers configured.',
  );
});

test('usage excludes local Ollama and explains unavailable cloud quotas', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [
          { id: 'c', name: 'Work', provider: 'claude' },
          { id: 'a', name: 'Google', provider: 'antigravity' },
          { id: 'o', name: 'Local', provider: 'ollama' },
        ],
        sessions: [],
      }),
    ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  const usage = page.getByRole('dialog', { name: 'Provider usage' });
  await expect(usage).toContainText('Work');
  await expect(usage).toContainText('Google');
  await expect(usage).not.toContainText('Local');
  await expect(usage).toContainText('use /usage in Claude Code');
  await expect(usage.getByLabel('No quota data available')).toHaveCount(2);
});

test('usage displays Codex windows and Antigravity model groups', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [
          { id: 'g', name: 'Codex', provider: 'gpt' },
          { id: 'a', name: 'Antigravity', provider: 'antigravity' },
        ],
        sessions: [],
        usage: [
          {
            accountId: 'g',
            updatedAt: Date.now(),
            source: 'codex-cli',
            windows: [
              { windowMinutes: 299, usedPercent: 23, resetsAt: Date.now() + 60000 },
              { windowMinutes: 1440, usedPercent: 67, resetsAt: Date.now() + 60000 },
              { windowMinutes: 43200, usedPercent: 42, resetsAt: Date.now() + 60000 },
            ],
          },
          {
            accountId: 'a',
            updatedAt: Date.now(),
            source: 'antigravity-cli',
            windows: [
              {
                group: 'Gemini Models',
                windowMinutes: 300,
                usedPercent: 10,
                resetsAt: Date.now() + 60000,
              },
              {
                group: 'Claude Models',
                windowMinutes: 10080,
                usedPercent: 55,
                resetsAt: Date.now() + 60000,
              },
            ],
          },
        ],
      }),
    ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  const usage = page.getByRole('dialog', { name: 'Provider usage' });
  for (const value of [
    '23%',
    '67%',
    '1 day',
    '42%',
    '30 day',
    'Gemini Models',
    '10%',
    'Claude Models',
    '55%',
  ]) {
    await expect(usage).toContainText(value);
  }
});

test('Claude usage resets at the reported deadline without a refresh request', async ({ page }) => {
  await page.clock.install({ time: new Date('2026-11-06T12:00:00Z') });
  await page.clock.pauseAt(new Date('2026-11-06T12:00:01Z'));
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [{ id: 'c', name: 'Claude', provider: 'claude' }],
        sessions: [],
        usage: [
          {
            accountId: 'c',
            updatedAt: Date.now(),
            source: 'claude-cli',
            windows: [{ windowMinutes: 300, usedPercent: 37, resetsAt: Date.now() + 5000 }],
          },
        ],
      }),
    ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  const usage = page.getByRole('dialog', { name: 'Provider usage' });
  await expect(usage).toContainText('37%');
  await page.clock.fastForward(5100);
  await expect(usage).toContainText('0%');
  await expect(usage).not.toContainText('37%');
});

test('first run, provider controls and honest browser boundary', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  await expect(
    page.getByRole('heading', { name: 'A clear space for a brighter idea.' }),
  ).toBeVisible();
  await expect(page.getByRole('button', { name: 'Build something' })).toHaveCount(0);
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Plan my project');
  await page.getByRole('button', { name: 'Send message' }).click();
  await expect(page.getByRole('alert')).toContainText('browser preview');
  await page.getByRole('button', { name: 'Claude', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Model', exact: true })).not.toBeVisible();
  await page.getByRole('button', { name: 'Advanced provider options' }).click();
  await expect(page.getByRole('combobox', { name: 'Model', exact: true })).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Model', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Auto', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Model', exact: true })).not.toBeVisible();
  expect(errors).toEqual([]);
  for (const text of [
    'A quieter kind of intelligence.',
    'Completely AI Written',
    'ALPHA',
    'On your computer. In your control.',
    'The right model, thoughtfully chosen.',
  ])
    await expect(page.getByText(text, { exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Both', exact: true })).toBeVisible();
  const permissions = page.getByRole('button', { name: /^Permissions:/ });
  for (let i = 0; i < 3; i += 1) await permissions.click();
  await expect(permissions).toHaveAttribute('aria-label', 'Permissions: Autopilot');
  await expect(page.getByRole('button', { name: 'Both', exact: true })).toHaveCount(0);
});

test('moving between projects preserves the conversation folder across reloads', async ({
  page,
}) => {
  await page.addInitScript(
    () =>
      !localStorage.getItem('lumen-preview-v1') &&
      localStorage.setItem(
        'lumen-preview-v1',
        JSON.stringify({
          accounts: [],
          workspaces: [
            { id: 'w1', name: 'Design workspace', folder: 'C:/Projects/Design' },
            { id: 'w2', name: 'Code workspace', folder: 'C:/Projects/Code' },
          ],
          sessions: [
            {
              id: 's',
              title: 'Linked conversation',
              status: 'idle',
              pinned: false,
              workspace: 'C:/Original',
              workspaceId: null,
              updatedAt: 1,
            },
          ],
        }),
      ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Linked conversation/ }).click();
  await page.getByRole('combobox', { name: 'Project', exact: true }).selectOption('w2');
  await expect(page.locator('.workspace-name')).toHaveText('Code workspace');
  await expect(page.locator('.workspace-name')).toHaveAttribute('title', 'C:/Original');
  await page.reload();
  await page.getByRole('button', { name: /^(?!Actions for).*Linked conversation/ }).click();
  await expect(page.getByRole('combobox', { name: 'Project', exact: true })).toHaveValue('w2');
  await expect(page.locator('.workspace-name')).toHaveAttribute('title', 'C:/Original');
});

test('accounts can be renamed from Settings without changing identity', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({ accounts: [{ id: 'g', name: 'Before', provider: 'gpt' }], sessions: [] }),
    ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Accounts' }).click();
  await page.getByRole('button', { name: 'Rename Before' }).click();
  await page.getByRole('textbox', { name: 'Account name' }).fill('After');
  await page.getByRole('button', { name: 'Save account name' }).click();
  await expect(page.getByRole('button', { name: 'Remove After' })).toBeVisible();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.getByRole('dialog').waitFor({ state: 'detached' });
  await page.getByRole('button', { name: 'GPT', exact: true }).click();
  await page.getByRole('button', { name: 'Advanced provider options' }).click();
  await expect(page.getByRole('combobox', { name: 'Account', exact: true })).toHaveValue('g');
  await expect(
    page
      .getByRole('combobox', { name: 'Account', exact: true })
      .getByRole('option', { name: 'After' }),
  ).toHaveCount(1);
});

test('settings, theme and support information persist across reloads', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Interface' })).toBeVisible();
  await page.getByRole('button', { name: 'Dark', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('tab', { name: 'About' }).click();
  await expect(page.getByRole('button', { name: 'Support Lumen on PayPal' })).toBeVisible();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('keyboard navigation and narrow layout stay usable', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Open sidebar' }).click();
  await expect(page.getByRole('textbox', { name: 'Search conversations' })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+n');
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toBeFocused();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('settings contains keyboard focus and restores it; search shortcut works', async ({
  page,
}) => {
  await page.goto('/');
  const settings = page.getByRole('button', { name: 'Settings', exact: true });
  await settings.focus();
  await page.keyboard.press('Enter');
  const panel = page.getByRole('dialog', { name: 'Settings panel' });
  const first = panel.getByRole('button', { name: 'Close settings', exact: true });
  await expect(first).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(first).not.toBeFocused();
  expect(await panel.evaluate((el) => el.contains(document.activeElement))).toBe(true);
  await page.keyboard.press('Tab');
  await expect(first).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(settings).toBeFocused();
  await page.keyboard.press('/');
  await expect(page.getByRole('textbox', { name: 'Search conversations' })).toBeFocused();
});

test('filtering the sidebar preserves the selected running conversation', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [],
        sessions: [
          {
            id: 'running-fixture',
            title: 'Running fixture',
            status: 'running',
            pinned: false,
            workspace: 'Fixture workspace',
            updatedAt: 1,
          },
        ],
      }),
    ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Running fixture/ }).click();
  await page.getByRole('textbox', { name: 'Search conversations' }).fill('no matches');
  await expect(page.getByText('No conversations found.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Stop response' })).toBeVisible();
  await expect(page.locator('.conversation-label')).toHaveText('Running fixture');
});

test('account errors remain visible inside settings', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Accounts' }).click();
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  await page.getByRole('textbox', { name: 'Account name' }).fill('Preview account');
  await page.getByRole('button', { name: 'Add account', exact: true }).last().click();
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('desktop app');
});

test('projects support shortcuts, context actions, drag and persistent Auto preferences', async ({
  page,
}) => {
  await page.addInitScript(() => {
    if (!localStorage.getItem('lumen-preview-v1'))
      localStorage.setItem(
        'lumen-preview-v1',
        JSON.stringify({
          accounts: [],
          workspaces: [],
          sessions: [
            {
              id: 'conversation',
              title: 'Move me',
              status: 'idle',
              pinned: false,
              workspace: 'C:/Work',
              updatedAt: 1,
            },
          ],
        }),
      );
  });
  await page.goto('/');
  await page.getByRole('textbox', { name: 'Message', exact: true }).click();
  for (const name of ['Project A', 'Project B']) {
    await page.keyboard.press('Control+p');
    await page.getByRole('textbox', { name: 'Project name', exact: true }).fill(name);
    await page.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(
      page.locator('.project-list').getByRole('button', { name, exact: true }),
    ).toBeVisible();
  }
  await page
    .locator('.project-list')
    .getByRole('button', { name: 'Unsorted', exact: true })
    .click();
  const actions = page.getByRole('button', { name: 'Actions for Move me' });
  await actions.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('menuitem', { name: 'Rename', exact: true })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(actions).toBeFocused();
  await page.keyboard.press('Enter');
  await page.getByRole('menuitem', { name: 'Rename', exact: true }).click();
  await page.getByRole('textbox', { name: 'Conversation title' }).fill('Renamed conversation');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  const row = page.locator('.session-row').filter({ hasText: 'Renamed conversation' });
  await row.click({ button: 'right' });
  await page
    .getByRole('combobox', { name: 'Move conversation to project' })
    .selectOption({ label: 'Project A' });
  await page
    .locator('.project-list')
    .getByRole('button', { name: 'Project A', exact: true })
    .click();
  await expect(row).toBeVisible();
  await row.dragTo(
    page.locator('.project-list').getByRole('button', { name: 'Project B', exact: true }),
  );
  await page
    .locator('.project-list')
    .getByRole('button', { name: 'Project B', exact: true })
    .click();
  await expect(row).toBeVisible();
  await row.click();
  await expect(page.locator('.workspace-name')).toHaveAttribute('title', 'C:/Work');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Advanced', exact: true }).click();
  await page.getByRole('tab', { name: 'Auto' }).click();
  await page.getByRole('button', { name: 'Budget', exact: true }).click();
  await page.getByRole('switch', { name: /Balance usage/ }).check();
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Auto' }).click();
  await expect(page.getByRole('button', { name: 'Budget', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.getByRole('switch', { name: /Balance usage/ })).toBeChecked();
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();
  await row.click();
  await expect(
    page.getByRole('combobox', { name: 'Project', exact: true }).locator('option:checked'),
  ).toHaveText('Project B');
  await row.click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
  await page.getByRole('button', { name: 'Delete conversation', exact: true }).click();
  await expect(row).toHaveCount(0);
});

test('settings sections and Simple/Advanced mode hide expert options', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const tabs = page.getByRole('tablist', { name: 'Settings sections' });
  await expect(page.getByRole('button', { name: 'Simple', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(tabs.getByRole('tab')).toHaveText([
    'General',
    'Accounts',
    'Auto',
    'Teamwork',
    'About',
  ]);
  await page.getByRole('tab', { name: 'Teamwork' }).click();
  await expect(page.getByRole('switch', { name: /Orchestra/ })).toBeVisible();
  await expect(page.getByRole('switch', { name: /Self-healing/ })).toHaveCount(0);
  await expect(page.locator('#settings-body').getByRole('switch')).toHaveCount(1);
  await page.getByRole('button', { name: 'Advanced', exact: true }).click();
  await page.getByRole('switch', { name: /Self-healing/ }).uncheck();
  await expect(tabs.getByRole('tab', { name: 'Tools' })).toBeVisible();
  await page.getByRole('button', { name: 'Simple', exact: true }).click();
  await expect(page.getByText('1 advanced option is on')).toBeVisible();
  await page.getByRole('tab', { name: 'General' }).click();
  await page.getByRole('switch', { name: /^Keep Lumen/ }).check();
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('switch', { name: /^Keep Lumen/ })).toBeChecked();
  await expect(page.getByText('Starts quietly in the notification area.')).toBeVisible();
  await page.getByRole('button', { name: 'Show', exact: true }).click();
  await page.getByRole('tab', { name: 'Teamwork' }).click();
  await expect(page.getByRole('switch', { name: /Self-healing/ })).not.toBeChecked();
});

test('Orchestra cards show questions, an editable plan and live progress', async ({ page }) => {
  const sub = (id: string, title: string, accountId: string, extra = {}) => ({
    id,
    title,
    detail: 'Do it',
    files: [`${id}.ts`],
    difficulty: 'normal',
    writes: true,
    dependsOn: [],
    accountId,
    agent: accountId === 'c' ? 'Claude · Sonnet' : 'ChatGPT · Terra',
    model: '',
    effort: 'low',
    status: 'pending',
    output: '',
    startedAt: 0,
    endedAt: 0,
    ...extra,
  });
  const state = (stage: string, more = {}) => ({
    stage,
    task: 'Build the feature',
    questions: [],
    confirm: '',
    answers: '',
    orchestrator: 'c',
    orchestratorAgent: 'Claude · Opus',
    orchestratorModel: 'opus',
    effort: 'medium',
    summary: 'Split API and UI',
    subtasks: [],
    planReview: '',
    readOnly: false,
    steps: [],
    lanes: [],
    ...more,
  });
  const msg = (id: string, st: object, text = '') => ({
    id,
    role: 'assistant',
    text,
    activity: [{ kind: 'orchestra', text: JSON.stringify(st) }],
  });
  await page.addInitScript(
    ([q, p, r, i]) =>
      localStorage.setItem(
        'lumen-preview-v1',
        JSON.stringify({
          accounts: [
            { id: 'c', name: 'Personal', provider: 'claude' },
            { id: 'g', name: 'Work', provider: 'gpt' },
            { id: 'o', name: 'Local', provider: 'ollama' },
          ],
          sessions: [
            {
              id: 'orc',
              title: 'Orchestra fixture',
              status: 'completed',
              pinned: false,
              workspace: 'C:/Work',
              updatedAt: 1,
            },
          ],
          messages: { orc: [q, p, r, i] },
        }),
      ),
    [
      msg(
        'q',
        state('questions', { questions: [{ q: 'Which database?', recommended: 'SQLite' }] }),
      ),
      msg('p', state('plan', { subtasks: [sub('s1', 'API', 'c'), sub('s2', 'UI', 'g')] })),
      msg(
        'r',
        state('running', {
          subtasks: [
            sub('s1', 'API', 'c', {
              status: 'done',
              output: 'API ready',
              startedAt: 1,
              endedAt: 2001,
            }),
            sub('s2', 'UI', 'g', { status: 'failed', output: 'Rate limit reached' }),
            sub('s3', 'Docs', 'g', { status: 'queued' }),
          ],
          steps: [{ name: 'Integration', status: 'running', detail: '' }],
          queue: {
            running: 0,
            admitted: 2,
            concurrent: 6,
            total: 2,
            waiting: [{ id: 's3', reason: 'total-cap' }],
            wakeAt: null,
          },
        }),
        'Final summary',
      ),
      msg(
        'i',
        state('interrupted', {
          resumable: true,
          notice: 'Could not verify that these subtasks stopped: Tests.',
          subtasks: [
            sub('s1', 'Lint', 'c', { status: 'done', output: 'Lint clean' }),
            sub('s2', 'Tests', 'g', {
              status: 'interrupted',
              output: 'Interrupted: Lumen was closed.',
            }),
          ],
        }),
      ),
    ],
  );
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Orchestra fixture/ }).click();
  const questions = page.getByRole('region', { name: 'Orchestra questions' });
  await expect(questions.getByRole('textbox', { name: /Which database/ })).toHaveValue('SQLite');
  await expect(questions.getByRole('button', { name: 'Skip questions' })).toBeVisible();
  const plan = page.getByRole('region', { name: 'Orchestra plan' });
  await expect(plan.getByText('Split API and UI')).toBeVisible();
  const agent = plan.getByRole('combobox', { name: 'Agent for API' });
  await expect(agent.locator('option[value="o"]')).toBeDisabled();
  await plan.getByRole('button', { name: 'Remove UI' }).click();
  await expect(plan.getByText('UI', { exact: true })).toHaveCount(0);
  await expect(plan.getByRole('button', { name: 'Remove API' })).toBeDisabled();
  await plan.getByRole('button', { name: 'Approve' }).click();
  await expect(page.getByRole('alert')).toContainText('desktop app');
  const progress = page.getByRole('region', { name: 'Orchestra progress' }).first();
  await expect(progress.getByText('2s')).toBeVisible();
  await expect(progress.getByLabel('Failed')).toBeVisible();
  await expect(progress.getByLabel('Queued')).toBeVisible();
  await expect(progress.getByText('Subtask limit reached: s3 (2 of 2 admitted)')).toBeVisible();
  await expect(progress.getByRole('button', { name: 'Allow 1 more' })).toBeVisible();
  await progress.getByText('API', { exact: true }).click();
  await expect(progress.getByText('API ready')).toBeVisible();
  await expect(page.getByText('Final summary')).toBeVisible();
  const resumed = page.getByRole('region', { name: 'Orchestra progress' }).last();
  await expect(resumed.getByText(/Completed subtasks are kept/)).toBeVisible();
  await expect(resumed.getByLabel('Interrupted')).toBeVisible();
  await expect(resumed.getByText(/could not verify/i)).toBeVisible();
  await expect(resumed.getByRole('button', { name: 'Resume anyway' })).toBeVisible();
  await resumed.getByRole('button', { name: 'Resume', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('desktop app');
});

test('Orchestra limits are optional and saved from Teamwork settings', async ({ page }) => {
  await page.addInitScript(() => {
    if (localStorage.getItem('lumen-preview-settings')) return;
    localStorage.setItem('lumen-preview-settings', JSON.stringify({ orchestra: true }));
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [
          { id: 'c', name: 'Personal', provider: 'claude' },
          { id: 'o', name: 'Local', provider: 'ollama' },
        ],
        sessions: [],
        messages: {},
      }),
    );
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Advanced', exact: true }).click();
  await page.getByRole('tab', { name: 'Teamwork' }).click();
  const limits = page.getByRole('group', { name: 'Orchestra limits' });
  const agents = limits.getByRole('spinbutton', { name: /Simultaneous agents/ });
  await expect(agents).toHaveAttribute('placeholder', '6');
  await agents.fill('2');
  await agents.press('Enter');
  // Wait until the first save is stored, so the next edit does not race it.
  const stored = () =>
    page.evaluate(() => JSON.parse(localStorage.getItem('lumen-preview-settings') ?? '{}'));
  await expect.poll(async () => (await stored()).orchestraLimits?.maxConcurrent).toBe(2);
  await expect(agents).toBeEnabled();
  const threshold = limits.getByRole('spinbutton', { name: /Claude · quota threshold/ });
  await expect(threshold).toBeEnabled();
  await threshold.fill('80');
  await threshold.press('Enter');
  await threshold.blur();
  await expect(limits.getByRole('spinbutton', { name: /Ollama/ })).toHaveCount(0);
  // Reloading while the threshold save is in flight would drop it, so wait until it is stored.
  await expect.poll(async () => (await stored()).orchestraLimits?.quotaThresholds?.claude).toBe(80);
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Teamwork' }).click();
  await expect(limits.getByRole('spinbutton', { name: /Simultaneous agents/ })).toHaveValue('2');
  await expect(limits.getByRole('spinbutton', { name: /Claude · quota threshold/ })).toHaveValue(
    '80',
  );
});

test('the Orchestra switch shows a composer badge that turns it off', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem('lumen-preview-settings', JSON.stringify({ orchestra: true })),
  );
  await page.goto('/');
  const badge = page.getByRole('button', { name: 'Orchestra', exact: true });
  await expect(badge).toBeVisible();
  await page.getByRole('button', { name: 'Claude', exact: true }).click();
  await expect(badge).toBeVisible();
  await page.getByRole('button', { name: 'Auto', exact: true }).click();
  await badge.click();
  await expect(badge).toHaveCount(0);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Teamwork' }).click();
  await expect(page.getByRole('switch', { name: /Orchestra/ })).not.toBeChecked();
});

test('choosing File edits with Antigravity connected warns once, unless hidden', async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [{ id: 'a', name: 'Antigravity', provider: 'antigravity' }],
        sessions: [],
      }),
    ),
  );
  await page.goto('/');
  const access = page.getByRole('button', { name: /^Permissions:/ });
  await access.click();
  const notice = page.getByRole('dialog', { name: 'Antigravity in File edits' });
  await expect(notice).toBeVisible();
  await notice.getByRole('checkbox', { name: "Don't show again" }).check();
  await notice.getByRole('button', { name: 'OK' }).click();
  await expect(notice).toHaveCount(0);
  for (let i = 0; i < 4; i += 1) await access.click();
  await expect(access).toHaveAttribute('aria-label', 'Permissions: Edits');
  await expect(notice).toHaveCount(0);
});

test('reasoning folds away under a chevron and the reply shows only the answer', async ({
  page,
}) => {
  await page.addInitScript(() => {
    if (localStorage.getItem('lumen-preview-v1')) return;
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [{ id: 'c', name: 'Personal', provider: 'claude' }],
        sessions: [
          {
            id: 'chat',
            title: 'Reasoning fixture',
            status: 'completed',
            pinned: false,
            workspace: 'C:/Work',
            updatedAt: 1,
          },
        ],
        messages: {
          chat: [
            { id: 'u', role: 'user', text: 'Question?', activity: [] },
            {
              id: 'a',
              role: 'assistant',
              text: '<think>weighing options</think>The answer is 42',
              activity: [{ kind: 'thinking', text: 'native thought' }],
            },
          ],
        },
      }),
    );
  });
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Reasoning fixture/ }).click();
  await expect(page.getByText('The answer is 42')).toBeVisible();
  await expect(page.getByText('weighing options')).toBeHidden();
  await expect(page.getByText('native thought')).toBeHidden();
  await page.getByText('Reasoning', { exact: true }).click();
  await expect(page.getByText('weighing options')).toBeVisible();
  await expect(page.getByText('native thought')).toBeVisible();
  await expect(page.getByText('Activity & diagnostics')).toHaveCount(0);
});

const contextFixture = (text: string) => {
  if (localStorage.getItem('lumen-preview-v1')) return;
  localStorage.setItem(
    'lumen-preview-v1',
    JSON.stringify({
      accounts: [{ id: 'c', name: 'Personal', provider: 'claude' }],
      sessions: [
        {
          id: 'chat',
          title: 'Context fixture',
          status: 'completed',
          pinned: false,
          workspace: 'C:/Work',
          updatedAt: 1,
        },
      ],
      messages: { chat: [{ id: 'u', role: 'user', text, activity: [] }] },
    }),
  );
};

test('the context meter shows a coloured dot, exact tokens and a Compact action', async ({
  page,
}) => {
  await page.addInitScript(contextFixture, 'x'.repeat(240_000));
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Context fixture/ }).click();
  // 240,000 characters is 60,000 tokens: 30% of a 200k window, so the dot is green.
  const meter = page.getByRole('button', { name: /^Context \d+%/ });
  await expect(meter).toHaveText(/Context 30%/);
  await expect(meter).toHaveClass(/context-meter low/);
  await meter.click();
  const popover = page.getByRole('dialog', { name: 'Context window' });
  await expect(popover.getByText(/60,0\d\d \/ 200,000/)).toBeVisible();
  await expect(popover.getByRole('progressbar', { name: 'Context used' })).toBeVisible();
  await expect(popover.getByText('Not measured')).toBeVisible();
  await popover.getByRole('button', { name: /Compact into a fresh session/ }).click();
  await expect(popover).toHaveCount(0);
  // The browser preview has no native backend, so the request surfaces an honest error.
  await expect(page.getByRole('alert')).toBeVisible();
});

test('the context dot turns orange and red at 50% and 80%', async ({ page }) => {
  await page.addInitScript(contextFixture, 'x'.repeat(680_000));
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Context fixture/ }).click();
  const meter = page.getByRole('button', { name: /^Context \d+%/ });
  await expect(meter).toHaveText(/Context 85%/);
  await expect(meter).toHaveClass(/context-meter high/);
});

test('the command palette opens with Ctrl K, filters, runs and restores focus', async ({
  page,
}) => {
  await page.goto('/');
  const composer = page.getByRole('textbox').first();
  await composer.focus();
  await page.keyboard.press('Control+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(palette).toBeVisible();
  await expect(palette.getByRole('combobox')).toBeFocused();
  await page.keyboard.type('settings: acc');
  await expect(palette.getByRole('option')).toHaveCount(1);
  await expect(palette.getByRole('option').first()).toHaveText(/Settings: Accounts/);
  await page.keyboard.press('Enter');
  await expect(palette).toBeHidden();
  await expect(page.getByRole('dialog', { name: /Settings/ }).first()).toBeVisible();
  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+k');
  await page.keyboard.type('zzzz');
  await expect(palette.getByRole('status')).toHaveText('No matching commands');
  await page.keyboard.press('Escape');
  await expect(palette).toBeHidden();
});

test('a new conversation can change its folder from the top bar', async ({ page }) => {
  await page.goto('/');
  const picker = page.locator('.topbar').getByRole('button', { name: /^Change folder/ });
  await expect(picker).toBeVisible();
  await picker.click();
  await expect(picker).toContainText('Browser preview');
});
