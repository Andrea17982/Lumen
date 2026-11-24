// PostToolUse hook: format a Rust file right after Claude edits it.
import { execFileSync } from 'node:child_process';

let input = '';
for await (const chunk of process.stdin) input += chunk;
const file = JSON.parse(input).tool_input?.file_path;
if (file?.endsWith('.rs')) {
  try {
    execFileSync('rustfmt', ['--edition', '2021', file], { stdio: ['ignore', 'ignore', 'pipe'] });
  } catch (error) {
    // Report syntax errors back to Claude instead of failing silently.
    process.stderr.write(`rustfmt failed on ${file}:\n${error.stderr ?? error.message}`);
    process.exit(2);
  }
}
