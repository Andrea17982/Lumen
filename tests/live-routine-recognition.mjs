// Live, opt-in: asks the installed Claude CLI (CLAUDE_CONFIG_DIR selects a Lumen account slot) whether
// Lumen's memory/routine hints are recognized. Uses real inference; run only when authorized.
import { readFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
const src = (f) => readFileSync(f, 'utf8');
const str = (s, name) => {
  const i = s.indexOf(`const ${name}: &str = "`) + `const ${name}: &str = `.length;
  let j = i + 1;
  while (s[j] !== '"') j += s[j] === String.fromCharCode(92) ? 2 : 1;
  return JSON.parse(s.slice(i, j + 1));
};
const hints =
  str(src('src-tauri/crates/lumen-core/src/context.rs'), 'MEMORY_HINT') +
  ' ' +
  str(src('src-tauri/crates/lumen-core/src/routines.rs'), 'HINT');
const cases = [
  [
    'recurring-it',
    'controlla ogni 30 minuti di aggiornare le dipendenze di questo progetto fino al 30 agosto 2027',
    true,
  ],
  ['recurring-en', 'Every Monday, summarize the open TODO comments in this repo.', true],
  ['one-off', 'Elenca le dipendenze di questo progetto.', false],
  ['memory', "D'ora in poi in questo progetto usiamo pnpm, non npm. Elenca le dipendenze.", false],
];
const dir = mkdtempSync(path.join(tmpdir(), 'lumen-live-'));
writeFileSync(
  path.join(dir, 'package.json'),
  JSON.stringify({ name: 'demo', dependencies: { react: '^18.0.0', lodash: '^4.17.0' } }, null, 2),
);
for (const [name, ask, want] of cases) {
  const prompt = `You are working in Lumen. Keep your native tools, agent behavior and project instructions. Complete the user's requested work within the current permission profile. Only claim checks that actually ran. ${hints} Read-only turn. Do not modify files.\n\nUser: ${ask}`;
  const out = execFileSync(
    'claude',
    ['-p', '--model', 'sonnet', '--allowedTools', 'Read,Glob,Grep'],
    { input: prompt, cwd: dir, encoding: 'utf8', shell: true, timeout: 300000 },
  );
  const routine = out.match(/<lumen-routine[^>]*>[\s\S]*?<\/lumen-routine>/)?.[0];
  const memory = out.match(/<lumen-remember>[\s\S]*?<\/lumen-remember>/g);
  console.log(
    `== ${name}: routine ${routine ? 'YES' : 'no'} (expected ${want ? 'YES' : 'no'}) ${routine ?? ''} | memory: ${memory ?? 'none'}`,
  );
}
