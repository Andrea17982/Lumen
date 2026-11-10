# Task continuity and code quality — 0.7

These backend features preserve the native execution workflow and add no repeated approval round. Ollama browsing uses the agreed official local API exception; subscription providers retain native CLI authentication.

## Durable task state

`workstate.rs` reconstructs a structured ledger from source messages and Lumen's execution evidence. SQLite's `work_states` table stores the goal, user requirements, decisions/approved scope, actual workspace and permission profile, finished agent tasks, pending tasks and check results.

- Facts carry source message IDs and preserve the user's text. Assistant prose such as “all tests passed” is not verification evidence.
- Full prompts, native CLI continuations and Orchestra worker handoffs receive a budgeted projection of the ledger. Full facts remain in SQLite and an app-data `work-state/<session-id>.json` context snapshot that tools can read when excerpts are insufficient. Snapshots are removed when the chat is deleted.
- Compact copies the ledger independently of the handoff prose and retains an edited handoff as the latest user-supplied decision. The target conversation's actual permissions override inherited approvals.
- Rewind reconstructs state from retained messages. An explicit `Nuovo obiettivo:`, `New task:` or `/task ` request starts a new scope without deleting history.
- Checks carry their workspace; another project's results are not evidence for the current project. Fresh results replace older results of the same check.
- `session_work_state` returns the full structured state for inspection.

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

Controlled tests use isolated projects and protocol peers. Real installed Rust/TypeScript/C#/Godot language servers, native Windows UI, Codex/Antigravity browser turns and the local Ollama loop have also been exercised. Claude/Gemini account-dependent and broader non-Windows desktop acceptance remain distinct. See [Validation](validation.md) for observed scope, platform builds and remaining limits.
