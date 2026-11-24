// Screen reader reach of a long transcript in the real WebView2 window (risk found in M0):
// with a 5,000-message session open at the bottom, asks Windows UI Automation whether the first,
// middle and last messages are exposed, by name and by their text. Opens a window for ~1 minute.
// Each message is named by its author and opening text. Prints the findings; fails if the first
// or last message cannot be reached. Text deep inside off-screen messages is a known gap.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { daemonClient, sleep } from '../daemon-harness.mjs';
import { daemonPath, launchApp, requireBuilds } from '../support/app.mjs';
import { seed, transcript } from '../support/transcript.mjs';

if (process.platform !== 'win32') {
  console.log(JSON.stringify({ skipped: 'UI Automation is Windows-only' }));
  process.exit(0);
}
requireBuilds();

const COUNT = 5000;
const data = await mkdtemp(path.join(tmpdir(), 'lumen-uia-'));
const workspace = path.join(data, 'workspace');
await mkdir(workspace);
const client = await daemonClient(data, daemonPath);
const daemon = client.start();
await client.ready();
await client.call('add_workspace', { name: 'Fixture', folder: workspace });
const chat = await client.call('create_session', { workspace });
await client.call('__shutdown');
await new Promise((resolve) => (daemon.exitCode !== null ? resolve() : daemon.once('exit', resolve)));
seed(data, chat.id, transcript(COUNT, 5));

// Chromium builds its UIA tree lazily: one full walk activates it, then each marker is searched
// by element name (a message group) and as text inside that group.
const probe = (pid) => `
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$T = [System.Windows.Automation.TreeScope]
$all = [System.Windows.Automation.Condition]::TrueCondition
$window = $A::RootElement.FindFirst($T::Children, (New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, ${pid})))
[void]$window.FindAll($T::Descendants, $all)
Start-Sleep -Seconds 3
$nodes = $window.FindAll($T::Descendants, $all)
$out = [ordered]@{ uiaNodes = $nodes.Count }
foreach ($marker in 'ZEBRAFIRST', 'ZEBRAMIDDLE', 'ZEBRALAST') {
  $named = @($nodes | Where-Object { $_.Current.Name -like "*$marker*" -and $_.Current.ControlType.ProgrammaticName -ne 'ControlType.Text' }).Count -gt 0
  $text = @($nodes | Where-Object { $_.Current.Name -like "*$marker*" -and $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Text' }).Count -gt 0
  $out[$marker] = [ordered]@{ byName = $named; asText = $text }
}
$out | ConvertTo-Json -Compress`;

let app;
try {
  app = await launchApp(data);
  const { browser } = app;
  await browser.until("return document.querySelector('[role=status]')?.textContent", (s) => s === 'Connected' || s === 'Connesso', 'connection');
  await browser.click('nav li button');
  await browser.until("return document.querySelectorAll('[role=log] article').length", (n) => n === COUNT, 'all messages mounted', 60_000);
  await sleep(1000);
  const result = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', probe(app.pid)], { encoding: 'utf8', windowsHide: true }));
  console.log(JSON.stringify({ messages: COUNT, ...result }, null, 2));
  for (const marker of ['ZEBRAFIRST', 'ZEBRALAST']) assert(result[marker].byName || result[marker].asText, `${marker} is not reachable through UI Automation`);
} finally {
  await app?.close();
  await client.call('__shutdown').catch(() => {});
  await sleep(500);
  await rm(data, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
}
