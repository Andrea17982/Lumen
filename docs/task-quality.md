# Task continuity and code quality

These backend features preserve native execution and add no repeated planning approval. Subscription providers retain native CLI authentication; local Ollama and the three official APIs use their declared tool loops. Action approvals still follow the selected profile, and paid API calls reserve budget before sending.

## Durable task state

`workstate.rs` reconstructs a structured ledger from source messages and Lumen's execution evidence. SQLite's `work_states` table stores the goal, user requirements, decisions/approved scope, actual workspace and permission profile, finished agent tasks, pending tasks and check results.

- Facts carry source message IDs and preserve the user's text. Assistant prose such as “all tests passed” is not verification evidence.
- Full prompts and Orchestra worker handoffs receive a budgeted projection of the ledger. Same-account native continuations keep the native transcript and send the new request, current permissions and changed Lumen-owned context rather than replaying the ledger every turn. Full facts remain in SQLite and an app-data `work-state/<session-id>.json` context snapshot that tools can read when excerpts are insufficient. Snapshots are removed when the chat is deleted.
- The generated handoff lists goal, constraints, decisions, done and open work, checks, changed files with Git diff, uncertainties and the next step. It states that the previous assistant's private memory and native session are not carried over.
- Compact copies the ledger independently of the handoff prose and retains an edited handoff as the latest user-supplied decision. The target conversation's actual permissions override inherited approvals.
- Rewind reconstructs state from retained messages. An explicit `Nuovo obiettivo:`, `New task:` or `/task ` request starts a new scope without deleting history.
- Checks carry their workspace; another project's results are not evidence for the current project. Fresh results replace older results of the same check.
- `session_work_state` returns the full structured state for inspection.

## Project memory

Every assistant can save a durable project fact (a decision, convention or preference) by writing `<lumen-remember>fact</lumen-remember>` in its final reply. After a successful turn Lumen removes the tag from the reply, adds the fact with its date and source chat (for example `_(2026-11-17, from “Release prep”)_`) under `## Lumen memory` in the project's `LUMEN.md` and shows "Remembered in LUMEN.md" under the message. Because `LUMEN.md` is already shared with every assistant, the next turn, a different agent or an API account sees the same fact.

Limits: at most five facts per turn, 300 characters each, exact duplicates skipped, text that looks like a credential rejected. Project facts stay in that project; other projects never see them. Edit or delete entries in `LUMEN.md` like any other note. The fact comes from the model's reply, so content the model read from an untrusted page could reach it; review the section like any other model output.

### Global memory

For a personal preference that applies everywhere, an assistant writes `<lumen-remember scope="global">fact</lumen-remember>`. Lumen removes the tag and stores nothing: the message shows "Remember for all projects?" with a Save button. Only after Save is the fact added, with date and source chat, to `shared/MEMORY.md` in Lumen's data folder, which every assistant in every project then receives; project notes win on conflict. Lumen saves only facts an assistant actually proposed in that reply, with the same size and credential filters. Edit or delete `MEMORY.md` freely. Both files are plain Markdown, so they are also the readable export: copy them to keep or move your memory.

Live check (17 November 2026, Claude Sonnet through the official CLI, `CASES=memory,global-it,global-en node tests/manual/live-routine-recognition.mjs`): 3/3. "In this project we use pnpm" stayed a project fact; "in all my projects I prefer short replies in Italian" and "across all my projects use 2-space indentation" were proposed with `scope="global"`.

## Recipes

The bookmark button under a finished reply saves that request and its accepted result as a shared skill named `recipe-…` in Lumen's shared skills folder. Every assistant receives shared skills, so later you can simply ask "repeat the release recipe" in any project or with another agent; the recipe tells the agent to redo the work for the current state rather than reuse old output. Saving never overwrites an existing recipe. Recipes are listed with the other skills in Settings; to edit or delete one, change or remove its `recipe-…` folder under `shared/skills` in Lumen's data folder. Request and result are stored as written, so do not save replies that contain secrets.

## Recurring tasks

Type `/every <interval> <task>` in an existing conversation, for example `/every 1d check for outdated dependencies and open a summary`. Intervals are `m`, `h`, `d` or `w`, between 15 minutes and 30 days. To align with the local clock use a time (`/every 08:00 …` runs daily at 08:00) or a weekday in English or Italian with an optional time (`/every lunedì 09:00 …`; without a time a weekday runs at 09:00). Aligned routines follow local wall-clock time across daylight-saving changes. The task is saved with the current assistant, model and effort choices and appears above the composer; × stops it.

The local daemon sends the task to the same conversation when it is due, also while the window is closed, with the conversation's current permission profile. A run is skipped while that conversation is already busy; runs missed while the computer was off are not replayed in a burst, and the next time is saved before the task starts, so a crash cannot submit it twice. Deleting the conversation deletes its routines.

You can also ask in plain words, for example "check every 30 minutes for dependency updates until 30 August 2027". The assistant does the first run and adds a `<lumen-routine every="30m" until="2027-08-30">` tag; Lumen hides the tag, creates one routine (duplicates of the same task are ignored), shows "Recurring task scheduled" under the reply and deletes the routine after the end date (end of that day, UTC). How well a request is recognized depends on the assistant; check the strip above the composer. A live check on 17 November 2026 with Claude Sonnet (`tests/manual/live-routine-recognition.mjs`) scheduled "every 30 minutes until 30 August 2027" (Italian), "every Monday", "ogni lunedì alle 9" (`mon` at 09:00) and "every day at 18:30", did not schedule a one-off request, and saved "use pnpm" as memory: 6/6 as expected. Use Read only or Ask first for unattended work you have not reviewed.

## Continuity and Auto calibration

Lumen adds a fingerprint for project notes and workspace-scoped backend checks to native-session metadata. A native continuation receives changed notes and the latest result per check label, with source message IDs and bounded details/diagnostics; unchanged context is omitted. At most eight recent labels are projected, with omitted coverage explicit. Removing context invalidates the old evidence rather than implying success. A switch to another account uses the full bounded handoff and this chat's check evidence; it never reuses another account's native session ID. This is per-chat continuity, not shared project-wide conversation history.

Auto reuses a previous verdict only for recognized short continuations such as “procedi”, not every short new task. A newer unrated manual request stops lookup of older verdicts. Fresh quota balancing takes precedence over native-account affinity; absent usable quota data retains the existing affinity behavior. Manual provider/model/effort choices remain authoritative.

Budget can request an independent audit for risky work or explicit user requests. Quality adds optional review for tier-two tasks, rather than every ordinary coding task. Explicit opt-out suppresses automatic review. Review prefers a different provider but can use a separate Claude/ChatGPT account on the same provider; Both still requires Claude plus ChatGPT. A requested audit without another primary account fails explicitly, and loss of an account after execution is not reported as an audit pass. Reviewers are read-only, and only a standalone `APPROVED` response accepts an audit; a qualified approval requires correction. An audit remains model feedback, not independent compile/runtime verification.

Reviewer-only calls do not replace the visible writer's resumable native metadata. If the writer falls back to another account, review selects an independent available account and chooses a proportional model from that account's actual catalog rather than reusing the old partner's model ID or an unrelated default. The native fixture checks actual writer identity and reviewer selection after fallback.

Difficulty alone no longer selects the Orchestra task-graph path: the fallback looks for implementation spanning distinct areas or a concrete request for parallel deliverables. Orchestra remains an explicit setting. These are bounded classifier/routing rules, not proof of smarter models or a new general delegation engine.

## Diagnostics and corrections

After a write turn, Lumen compares the working tree with that turn's checkpoint. This detects new files and edits even without provider `file` events and excludes unchanged pre-existing work. Provider-reported files are the fallback without a Git checkpoint.

For ordinary turns, additional Lumen validation is opt-in (`verifyChanges`, default false); the native harness can still run its own checks. Integrated Orchestra results retain final validation. When enabled, validation:

1. Queries applicable installed language servers for fresh diagnostics on changed/affected documents.
2. Runs relevant headless checks in their project directories.
3. Persists structured results and shows readable verification activity.
4. Sends concrete errors, task context and impact information back to the writer, with at most three corrections and a fresh validation after each one.
5. Leaves an error state if known failures remain. Missing/incomplete coverage is explicit, never a pass.

Disabling Self-healing disables corrections, not validation. Read-only turns do not start automatic checks. In Ask-first mode backend checks are deferred to the agent's approved tools. Servers are not installed automatically and playtests are never started automatically.

### Language servers

The client supports Content-Length-framed JSON-RPC, initialize/initialized, document open/close, push diagnostics and negotiated diagnostic pulls. It accepts diagnostics only for documents opened by this validation run, ignores explicitly stale versions, rejects requested edits and bounds message sizes, documents and timeouts. A received diagnostic snapshot is not full-project compilation.

Default discovery uses `rust-analyzer`, `typescript-language-server --stdio` and `csharp-ls` on PATH. GDScript needs explicit configuration, normally the editor's loopback TCP language server. A rustup proxy alone does not prove the rust-analyzer component is installed.

Optional `.lumen/diagnostics.json` replaces the default list:

```json
{
  "servers": [
    {
      "name": "Rust",
      "command": "rust-analyzer",
      "extensions": ["rs"],
      "initializationOptions": {"checkOnSave": false}
    },
    {
      "name": "TypeScript",
      "command": "typescript-language-server",
      "args": ["--stdio"],
      "extensions": ["ts", "tsx", "js", "jsx"]
    },
    {
      "name": "GDScript",
      "address": "127.0.0.1:6005",
      "extensions": ["gd"],
      "root": "game",
      "timeoutSeconds": 15
    }
  ]
}
```

Use the actual editor address and project directory; the example does not connect automatically. Optional `root` must stay inside the selected workspace; otherwise the nearest relevant manifest determines the root. TCP endpoints must be loopback. Lumen closes opened documents but does not shut down an externally owned TCP server.

Limits: eight servers, four roots per server, 80 selected paths, 24 documents per root, 512 KB per document, 2 MB per protocol message and 1–30 seconds for a server's diagnostic exchange. Omitted coverage is explicit.

Protocol reference: [Microsoft's LSP specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/).

### Project checks

Selection uses affected files and nearest manifests: Rust compile/tests, JavaScript package typecheck/check/build and unit/test scripts, C# compilation, Go build/tests and Godot resource checks. `verify:` in `LUMEN.md` remains supported, with the detected default as a fallback. `verify: off` disables automatic shell checks.

Custom `.lumen/checks.json` commands run automatically only with `automatic: true` and without `playtest: true`:

```json
[
  {
    "name": "Domain regression tests",
    "command": "npm run test:domain",
    "cwd": "backend",
    "automatic": true,
    "playtest": false
  }
]
```

The bounded runner uses `CI=true`, project-contained working directories and process-tree cancellation. Eight selected checks run per validation pass; additional checks are reported as unavailable coverage. Output is bounded before entering correction prompts.

## Impact connected to workers and tests

Structured impact includes targets, direct/transitive dependents, test candidates, shared signatures, unknown targets and partial coverage. The graph remains pattern-based, not a complete semantic parser.

- With Code graph enabled, a pre-edit snapshot is taken under the project write lock. Post-edit analysis merges old/current graphs so deleting or renaming shared code does not erase its former callers.
- Orchestra workers receive their owned files' contracts, possible affected callers/tests and relevant checks. Discovering another agent's necessary change does not authorize editing that agent's files.
- Integration receives the same context; final validation includes affected project roots, not only the edited package.
- Unknown files and incomplete indexing are explicit. No discovered dependents is not proof a change is safe.
- Index timestamps retain nanosecond precision and graph keys include file sizes. Index version 3 refreshes older caches.

## Verification scope

Automated tests cover persistence/reopen/Compact seeding/Rewind, decisions beyond the transcript cutoff, permissions, workspace-scoped evidence, LSP push/pull and stale versions, read-only callbacks, absent servers/timeouts, actual failing/passing shell checks, cancellation, nested checks, checkpoint detection and pre/post-edit impact in worker handoffs.

Controlled tests use isolated projects and protocol peers. Real installed Rust/TypeScript/C#/Godot language servers, native Windows UI, Codex/Antigravity browser turns and the local Ollama loop have also been exercised. Claude account-dependent and broader non-Windows desktop acceptance remain distinct. See [Validation](validation.md) for observed scope, platform builds and remaining limits.
