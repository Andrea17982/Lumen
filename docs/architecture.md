# Architecture

## Boundaries

Tauri 2 owns the desktop shell and command boundary. Rust owns session storage, accounts, process creation, routing, cancellation, concurrency and execution stages. React renders state and subscribes to normalized events. The browser UI has no shell or filesystem plugin permission and makes no inference HTTP requests.

```text
src/
  main.tsx                     React entry
  App.tsx                      Workspace shell: session rail, top bar, transcript, composer
  styles.css                   Theme tokens, glass material, layout, motion
  lib/
    bridge.ts                  Typed IPC, run-event reconciliation, honest UI preview
    bridge.test.ts             Streaming reconciliation tests
    types.ts                   Frontend data contracts
    util.tsx                   Shared constants and helpers (links, provider names, mark)
  components/
    MessageBody.tsx            Safe markdown rendering (lazy loaded)
    ProviderOptions.tsx        Official model/effort pickers and account renaming
    ProjectPanel.tsx           LUMEN.md notes and file attachments
  settings/
    SettingsPanel.tsx          Settings overlay: layout, appearance, folder, about
    AccountSettings.tsx        Connected accounts, sign-in, CLI detection
    AutoSettings.tsx           Manual Claude quota snapshot
    AutomationSettings.tsx     Teamwork switches
    IntegrationSettings.tsx    MCP hub and shared skills
src-tauri/
  src/main.rs          Desktop entry
  src/lib.rs           Tauri commands, settings, headless API turns, PATH fix
  src/turns.rs         Turn orchestration: single, cross-audit, parallel, Both,
                       rate-limit fallback, self-healing, Rewind, handoff
  src/projecttools.rs  Project-scoped skill/MCP discovery, Godot diagnostics,
                       bounded checks and source-location opening
  src/repomap.rs       Incremental symbol index (C#, GDScript, TS/JS, SQL, scenes)
                       and the bounded prompt block of mentioned declarations
  src/godot.rs         Godot MCP connection test over stdio, editor launch
  src/assets.rs        Built-in check for missing res:// references
  src/layers.rs        Uncommitted changes grouped by layer, with covering checks
  src/screens.rs       Screen/window capture, import and labelled screenshots
  src/approvals.rs     Ask-first Claude turns (permission-prompt-tool + MCP bridge)
  src/approvals_codex.rs  Ask-first Codex turns over `codex app-server`
  src/gitops.rs        Private-index checkpoints, restores and lane worktrees
  src/context.rs       LUMEN.md, file walker, attachments, log spill, verify
  src/workstate.rs     Durable source-linked task ledger, prompt budgets, Compact state
  src/workspace.rs     Explicit conversation-root changes before tools and planning
  src/diagnostics.rs   Bounded stdio/loopback LSP diagnostics, without applying edits
  src/validation.rs    Changed/affected project checks and recorded verification
  src/impact.rs        Pre/post-edit graph impact and worker/check handoffs
  src/limits.rs        Rate-limit detection, cooldowns, turn counters
  src/api.rs           OpenAI-compatible localhost server (axum)
  src/settings.rs      Persisted backend preferences
  src/integrations.rs  Shared skills and MCP hub adapters
  src/usage.rs         Codex quota query
  src/models.rs        Serializable domain types
  src/store.rs         SQLite schema and persistence operations
  src/router.rs        Pure deterministic Auto planner
  src/catalog.rs       Official CLI metadata discovery, pagination and validation
  src/providers.rs     CLI discovery, isolation, spawn, JSONL normalization, kill
  capabilities/        Window capabilities, folder picker, URL allowlist
  tauri.conf.json      Window, CSP, build and bundle settings
tests/                 Playwright UI tests, native offline smoke test, CLI fixtures
docs/                  Setup, accounts, features, policy, roadmap and development
.github/workflows/      ci.yml (checks on Windows, macOS, Ubuntu) · release.yml (Windows + macOS builds)
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
6. Continue supported native CLI sessions or replay bounded transcript context. Add the durable task ledger, project notes and the actual workspace/permission profile; full task facts remain retrievable independently of transcript truncation.
7. Spawn the official CLI, pass prompt over stdin, drain stderr without publishing raw auth diagnostics, normalize JSONL stdout.
8. Persist text/activity checkpoints and emit `lumen:run` events carrying session/message IDs.
9. Complete remaining reviewer/parallel/synthesis stages if planned.
10. Validate changed/affected code and perform bounded corrections where permitted. Save final output and completed/error/interrupted state; release workspace/job slots. Known unresolved validation failures leave an error state.

No shell command is assembled from a prompt. The prompt is stdin. Model identifiers use an allowlist of characters. Windows npm `.cmd` shims are invoked through `cmd.exe`; native executables are invoked directly. Cancellation uses a watch channel, kills the CLI process tree on Windows, and reaps the direct child. Unix uses a process group and SIGKILL. On app exit active jobs receive cancellation; abrupt termination of the application can leave descendants, so crash recovery marks persisted work interrupted rather than claiming clean shutdown.

## UI and event contract

Events distinguish start, status, delta, replace, thinking, tool, file, review, error and done. Claude emits partial text; Codex emits completed text items via its documented exec JSONL format. The UI never claims access to private chain-of-thought: it displays only content the provider exposes. Markdown disables raw HTML, remote images and active external links in model content; external support links use a narrow opener allowlist.

Frontend state is intentionally small: active session, visible messages, account metadata, mode, optional overrides and settings state. Auth material never crosses IPC. Settings use a right-side dialog with keyboard focus containment/restoration and inert background controls. Theme follows the system unless explicitly overridden.

## Conventions

New UI goes in `components/` (conversation surface) or `settings/` (one file per Settings section); shared helpers in `lib/`. New backend features get their own Rust module wired from `lib.rs` (commands) and `turns.rs` (turn stages). Keep `App.tsx` as the shell: state and layout, not feature logic.

## Extension points

Add providers by extending `Provider` (and its `fallback_rank`), command construction, normalization fixtures and routing (see `providers.rs`, `catalog.rs`, `router.rs`; `ollama.rs` is the smallest example).

## Changes after 0.4.5

- The composer and turn lifecycle do not capture screenshots automatically. Claude Ask-first's session-bound MCP endpoint exposes `list_screens` and `capture_screen`; captures are scoped to that turn's working directory, saved under `.lumen/screenshots/`, and returned as MCP image content. User-started Visual review still uses the Tauri commands in `screens.rs`.
- `reset_settings` restores `AppSettings::default()` and disables tray/autostart preferences. UI preferences (language, theme, text size, Auto policy and usage balance) reset in General settings. Provider accounts, conversations and workspace selection are kept.

- `repomap.rs` keeps one index per project in `<app-data>/repomap/<hash>.json`, never in the project or `LUMEN.md`. Files are walked with the same ignore rules as attachments (`.gitignore`, hidden folders, secrets, noise) and re-read only when size or modification time changes; deleted and renamed files drop out on the next refresh. Definitions come from per-language patterns, not a parser: it is a map of declarations, not a call graph. Limits: 20,000 files, 512 KB per file, 8 s per refresh. `turns::build_prompt` adds the declarations of identifiers the latest request names (at most 12, 2,400 characters) when `repoContext` is on and records them as a `references` activity on that request. Every path that builds its prompt there (single, Both, Orchestra's orchestrator, Ollama) gets the same block.
- Context meter: the Claude parser keeps each request's input (`usage`) and the `contextWindow` of the result event's `modelUsage`. `turns::stage` stores one `usage` activity per reply as JSON `{latest, overhead, window}`, where `overhead` is the first request's input minus the local prompt estimate. The meter adds that overhead to the local estimate of the next prompt and uses the reported window. Other providers stay local estimates over published windows.
- `godot.rs` starts a Godot server declared in `.mcp.json` or `.codex/config.toml` only on request, sends `initialize`, `tools/list` and `get_project_info`, then stops it. Arguments and environment values are expanded for the child and never returned.
- `screens.rs` saves captures and imports in `<project>/.lumen/screenshots/` with a JSON sidecar (label, resolution, source, time); the folder carries its own `.gitignore`. Image attachments are sent as a file reference the CLI opens with its own tools, not inlined.
- `approvals_codex.rs` follows the Gatekeeper README's runner steps: schema and `initialize` evidence from the selected binary, `thread/start` and `turn/start`, a reader that never waits on approvals, one writer queue, `delivery_is_current` before each reply and `complete_delivery` after it. Requests the adapters cannot represent are declined.

## Changes in 0.4

- `orchestra.rs` runs Orchestra. Its state is one `orchestra` activity (JSON) on each assistant message, replaced in place and mirrored by `orchestra` events; subtask output streams as `orchestra-delta` events that the UI keeps out of the saved transcript. Follow-up steps (`orchestra_answer`, `orchestra_approve`) run through `turns::spawn_turn`, so they use the same job slots, cancellation and status as ordinary turns. Subtask worktrees live under `<app-data>/worktrees/<id>/<subtask>` on `lumen/orchestra-*` branches.
- `projecttools.rs` reads project-local skill and MCP declarations without exposing server arguments, headers or environment values. It lists project checks but runs them only on request with bounded output. Godot diagnostics find nested project files and a Godot executable; the scan itself starts nothing (the connection test is in `godot.rs`).
- `ollama.rs` lists local models and exposes a one-model catalog; `providers.rs` runs `ollama run` and reads plain text.
- `tray.rs` owns the tray icon, close-to-tray, the quit confirmation event and the login item (`tauri-plugin-autostart`, `tauri-plugin-notification`). The main window starts hidden and is shown in `setup` unless Lumen was started at login with the tray on.
- Settings fields: `orchestra`, `closeToTray`, `openAtLogin`, `ollamaModel`, `trayHintShown`. `src/lib/settings.ts` shares one copy between Settings and the composer.

## Changes in 0.2

SQLite adds a workspaces table and a nullable sessions.workspace_id through a backward-compatible migration. The existing sessions.workspace field remains the canonical conversation folder, which can differ from the workspace default. Account renaming updates only metadata and preserves its UUID/configuration directory.

The catalog module initializes Codex app-server and pages model/list, or initializes the Claude streaming control protocol and reads models from its initialization response. Both are metadata operations without a user turn. Models/efforts are cached per account for five minutes, can be refreshed, and never require Lumen to inspect credentials. Cancellation/timeout reaps the metadata process tree. CLI catalogs do not guarantee model access for an inference request.

Permissions are typed ReadOnly/FileEdits/Autopilot profiles; old writeAccess IPC input remains compatible. Autopilot applies to writer/synthesis stages. Reviewers and parallel analysis remain read-only. See the dedicated permissions guide for exact flags and OS boundaries.
