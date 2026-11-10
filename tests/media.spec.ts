import { expect, test } from '@playwright/test';

test('Moli image activity renders its path and browser activity stays readable', async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [],
        sessions: [
          {
            id: 'moli',
            title: 'Moli image',
            status: 'completed',
            workspace: 'C:/Work',
            updatedAt: 1,
          },
        ],
        messages: {
          moli: [
            {
              id: 'moli-answer',
              role: 'assistant',
              text: 'Page captured.',
              activity: [
                { kind: 'browser', text: 'Moli · browser_screenshot\nCurrent page' },
                {
                  kind: 'image',
                  text: JSON.stringify({
                    id: 'shot',
                    path: 'https://example.test/moli-shot.png',
                    folder: 'C:/Worker',
                  }),
                },
              ],
            },
          ],
        },
      }),
    ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Moli image/ }).click();
  await expect(page.getByRole('button', { name: 'Load image: moli-shot.png' })).toBeVisible();
  await expect(page.locator('.prose')).not.toContainText('"folder"');
});
test('layout constraints support keyboard editing, persistence and removal', async ({ page }) => {
  await page.addInitScript(
    () =>
      localStorage.getItem('lumen-preview-v1') ||
      localStorage.setItem(
        'lumen-preview-v1',
        JSON.stringify({
          accounts: [],
          sessions: [
            {
              id: 'layout',
              title: 'Canvas test',
              status: 'completed',
              workspace: 'C:/Canvas',
              updatedAt: 1,
            },
          ],
          messages: {},
        }),
      ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Canvas test/ }).click();
  await page.getByRole('button', { name: 'Project inspector', exact: true }).click();
  const inspector = page.getByRole('complementary', { name: 'Project inspector' });
  await inspector.getByRole('button', { name: 'Layout canvas', exact: true }).click();
  await inspector.getByRole('button', { name: 'Add block' }).click();
  await inspector.getByLabel('Block label').fill('Navigation');
  const block = inspector.getByRole('button', { name: /^Navigation, content/ });
  await block.focus();
  await page.keyboard.press('Shift+ArrowRight');
  await expect(inspector.getByLabel('x', { exact: true })).toHaveValue('30');
  await inspector.getByLabel('Constraints').fill('Keep the navigation visible');
  await inspector.getByRole('button', { name: 'Save layout' }).click();
  await expect(inspector.getByRole('status')).toContainText('Layout saved');
  await page.reload();
  await page.getByRole('button', { name: /^(?!Actions for).*Canvas test/ }).click();
  await page.getByRole('button', { name: 'Project inspector', exact: true }).click();
  await inspector.getByRole('button', { name: 'Layout canvas', exact: true }).click();
  await inspector.getByRole('button', { name: /^Navigation, content/ }).click();
  await expect(inspector.getByLabel('Constraints')).toHaveValue('Keep the navigation visible');
  await inspector.getByRole('button', { name: 'Remove block' }).click();
  await inspector.getByRole('button', { name: 'Save layout' }).click();
  await expect(inspector.locator('.layout-block')).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem('lumen-preview-v1') || '{}').layouts['C:/Canvas'].blocks,
    ),
  ).toEqual([]);
});
test('response images load on request, enlarge and return keyboard focus', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [{ id: 'c', name: 'Claude', provider: 'claude' }],
        sessions: [
          {
            id: 'media',
            title: 'Media fixture',
            status: 'completed',
            workspace: 'C:/Work',
            updatedAt: 1,
          },
        ],
        messages: {
          media: [
            {
              id: 'answer',
              role: 'assistant',
              text: '![Diagram](http://127.0.0.1:1420/response-image.png)\n\n[Source](file:///C:/Work/source.ts:9)',
              activity: [],
            },
          ],
        },
      }),
    ),
  );
  let requests = 0;
  await page.route('**/response-image.png', (route) => {
    requests++;
    return route.fulfill({
      contentType: 'image/png',
      body: Buffer.from(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=',
        'base64',
      ),
    });
  });
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Media fixture/ }).click();
  await expect(page.getByRole('button', { name: 'Load image: Diagram' })).toBeVisible();
  expect(requests).toBe(0);
  await page.getByRole('button', { name: 'Load image: Diagram' }).click();
  const enlarge = page.getByRole('button', { name: 'Enlarge image: Diagram' });
  await expect(enlarge).toBeVisible();
  await enlarge.click();
  await expect(page.getByRole('dialog', { name: 'Diagram' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toBeHidden();
  await expect(enlarge).toBeFocused();
  expect(requests).toBeGreaterThan(0);
  await expect(page.getByRole('button', { name: 'Source', exact: true })).toBeVisible();
});
test('inspector is keyboard reachable, rejects remote addresses and restores focus', async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [],
        sessions: [
          {
            id: 'project',
            title: 'Inspector fixture',
            status: 'completed',
            workspace: 'C:/Work',
            updatedAt: 1,
          },
        ],
        messages: {},
      }),
    ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Inspector fixture/ }).click();
  const trigger = page.getByRole('button', { name: 'Project inspector', exact: true });
  await trigger.click();
  const inspector = page.getByRole('complementary', { name: 'Project inspector' });
  await expect(inspector).toBeVisible();
  await inspector.getByLabel('Preview address').fill('https://example.com');
  await inspector.getByRole('button', { name: 'Load preview' }).click();
  await expect(inspector.getByRole('alert')).toContainText('localhost');
  await expect(inspector.locator('iframe')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(inspector).toBeHidden();
  await expect(trigger).toBeFocused();
});
