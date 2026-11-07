# Feature integration status

Orchestra scheduling is integrated: the native runner in
`src-tauri/src/orchestra.rs` admits and assigns every subtask through
`src-tauri/src/scheduling.rs`, using the limits saved in Settings → Teamwork, and
publishes the queue shown by `QueueNotice`. Outdated quota snapshots still count
until their windows reset and are re-checked with one job first.

Restart recovery is integrated in `orchestra.rs`: a run interrupted while its
subtasks worked keeps completed results (with their patches), attempt counts and
the base tree, and waits for Resume. Resume stops the CLIs the previous session
left running (PID plus start time, so a reused PID is left alone), removes the
interrupted lanes and reruns only the unfinished subtasks; an unverifiable
process requires "Resume anyway". Runs interrupted during questions, planning,
integration or the final check are closed, not resumed.

The local helper is integrated: titles, commit-message drafts (copied, never
committed) and an editable handoff draft that the compact flow sends as the new
conversation's summary.

Ask-first Claude turns are integrated: `src-tauri/src/approvals.rs` starts the
Claude CLI with the Gatekeeper MCP bridge, probes readiness with no prompt, then
runs the turn with `--permission-prompt-tool`. Approvals appear in the chat's
panel and are decided through the native broker. Ask-first Codex turns run over
`codex app-server` in `src-tauri/src/approvals_codex.rs`, which maps the app's `gpt`
provider to Gatekeeper's `codex`. Orchestra subtasks do not use Ask-first.

## Shared integration contract

- Orchestra scheduling uses the app provider ID `gpt`; Gatekeeper uses `codex`
  for that CLI's protocol. Map these explicitly in the native runner, keeping
  account IDs unchanged. Gatekeeper supports only Claude and Codex; its enabled
  execution path must not silently switch to a provider without approval support.
- The Rust scheduler is the only execution authority; the frontend keeps just the
  limit settings and the queue description.
- A total-subtask cap counts admissions across the whole run. Finishing work
  frees concurrency only; increasing the total requires explicit user action.
- The local helper has independent opt-in settings and a local model. Its failures
  must preserve manual editing without invoking Orchestra's provider fallback.
  Chat titles currently start with app defaults; the host must track user renames
  and translate an untouched default title into the helper's untitled condition,
  then recheck ownership of the title when generation completes.

See the gatekeeper and local-assistant READMEs for native lifecycle and
UI wiring. Scheduling UI lives in `orchestra/scheduling/`, the scheduler
in `src-tauri/src/scheduling.rs`.

## Acceptance still open

- Integrated Orchestra run on an isolated copy of Lumen, including quota changes,
  queued work, restart, repeated Resume and prevention of duplicate effects.
- Ask-first in Orchestra subtasks.
- Ask-first in the native window: approve, reject and cancel clicks and the
  countdown are not yet verified in the webview. The real Claude turn test
  decided through the broker directly.
- Clean-machine installation on Windows and native macOS/Linux checks, including
  macOS login launch behavior. Autostart already exists; no hardening was applied.
