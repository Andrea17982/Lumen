# Architecture

## Boundaries

Tauri 2 owns the desktop shell and command boundary. Rust owns session storage, accounts, process creation, routing, cancellation, concurrency and execution stages. React renders state and subscribes to normalized events. The browser UI has no shell or filesystem plugin permission and makes no inference HTTP requests.

```text
src/
  main.tsx             React entry
  App.tsx              Workspace, session rail, composer, settings, activity
  bridge.ts            Typed IPC, run-event reconciliation, honest UI preview
  types.ts             Frontend data contracts
  ProviderOptions.tsx  Official model/effort pickers and account renaming
  styles.css           Theme tokens, glass material, layout, motion
  bridge.test.ts       Streaming reconciliation tests
src-tauri/
  src/main.rs          Windows desktop entry
  src/lib.rs           Tauri commands, job lifecycle, stage orchestration
  src/models.rs        Serializable domain types
  src/store.rs         SQLite schema and persistence operations
  src/router.rs        Pure deterministic Auto planner
  src/catalog.rs       Official CLI metadata discovery, pagination and validation
  src/providers.rs     CLI discovery, isolation, spawn, JSONL normalization, kill
  capabilities/        Window capabilities, folder picker, URL allowlist
  tauri.conf.json      Window, CSP, build and NSIS bundle settings
tests/                 Playwright UI behavior tests
docs/                  Setup, accounts, policy, roadmap and development
```

## Storage

Tauri resolves an application-specific data directory using identifier `io.github.andrea17982.lumen`. `lumen.db` uses SQLite WAL and foreign keys. Tables: sessions (identity, title, pin, state, canonical workspace, updated timestamp), messages (role, text, activity JSON, ordering), accounts (label, provider, generated UUID). Accounts never contain passwords or OAuth tokens.

The session list is paginated in groups of 100. Messages load for the selected conversation. This avoids rendering all historical conversations, although the complete selected transcript currently loads into memory. Tool activity is bounded to 500 retained events per worker message, with each textual event capped to 16,000 characters. Extremely large conversation histories still need the virtualization planned for v1.

Provider-owned configuration lives at `accounts/<generated UUID>/`. The child receives only its directory through `CLAUDE_CONFIG_DIR` or `CODEX_HOME`. Inherited common API-key / token environment variables are removed so a slot cannot silently use the parent process's identity. Codex is configured to use file-backed auth within its own home. CLI-managed credential-store behavior still needs testing across versions, particularly Claude on platforms using a shared OS store.

## Turn lifecycle

1. Validate nonempty text, size, session, model identifier and provider options.
2. Compute a plan locally. Reject duplicate turns in the same session.
3. Save the user message, generate a short first-message title if necessary, mark waiting.
4. Acquire one of four job slots; queued work remains cancellable.
5. For file-writing jobs, acquire a mutex keyed by canonical workspace path. Windows paths are normalized for case when used as lock keys. This coordinates Lumen turns, not external editors.
6. Replay up to 100,000 recent characters of transcript as context. This alpha does not resume a provider-native thread.
7. Spawn the official CLI, pass prompt over stdin, drain stderr without publishing raw auth diagnostics, normalize JSONL stdout.
8. Persist text/activity checkpoints and emit `lumen:run` events carrying session/message IDs.
9. Complete remaining reviewer/parallel/synthesis stages if planned.
10. Save final output and completed/error/interrupted state; release workspace/job slots.

No shell command is assembled from a prompt. The prompt is stdin. Model identifiers use an allowlist of characters. Windows npm `.cmd` shims are invoked through `cmd.exe`; native executables are invoked directly. Cancellation uses a watch channel, kills the CLI process tree on Windows, and reaps the direct child. Unix uses a process group and SIGKILL. On app exit active jobs receive cancellation; abrupt termination of the application can leave descendants, so crash recovery marks persisted work interrupted rather than claiming clean shutdown.

## UI and event contract

Events distinguish start, status, delta, replace, thinking, tool, file, review, error and done. Claude emits partial text; Codex emits completed text items via its documented exec JSONL format. The UI never claims access to private chain-of-thought: it displays only content the provider exposes. Markdown disables raw HTML, remote images and active external links in model content; external support links use a narrow opener allowlist.

Frontend state is intentionally small: active session, visible messages, account metadata, mode, optional overrides and settings state. Auth material never crosses IPC. Settings use a right-side dialog with keyboard focus containment/restoration and inert background controls. Theme follows the system unless explicitly overridden.

## Extension points

Add providers by extending Provider, command construction, normalization fixtures and routing. Move orchestration behind a trait when adding testable alternate process transports. v1 should split App into independent session/composer/settings components, version the SQLite schema, add message pagination, native provider resumption, account diagnostics and validated model catalogs. v2 can introduce budget-aware scheduling and isolated worktree workers.

## Changes in 0.2

SQLite adds a workspaces table and a nullable sessions.workspace_id through a backward-compatible migration. The existing sessions.workspace field remains the canonical conversation folder, which can differ from the workspace default. Account renaming updates only metadata and preserves its UUID/configuration directory.

The catalog module initializes Codex app-server and pages model/list, or initializes the Claude streaming control protocol and reads models from its initialization response. Both are metadata operations without a user turn. Models/efforts are cached per account for five minutes, can be refreshed, and never require Lumen to inspect credentials. Cancellation/timeout reaps the metadata process tree. CLI catalogs do not guarantee model access for an inference request.

Permissions are typed ReadOnly/FileEdits/Autopilot profiles; old writeAccess IPC input remains compatible. Autopilot applies to writer/synthesis stages. Reviewers and parallel analysis remain read-only. See the dedicated permissions guide for exact flags and OS boundaries.
