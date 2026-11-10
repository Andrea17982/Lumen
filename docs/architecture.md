# Architecture — 0.7

Tauri owns the native window and IPC boundary. Rust owns storage, account configuration, process/API execution, permissions, routing and cancellation. React renders normalized events. The webview does not read provider credentials or contact inference services directly.

## Execution modules

| Module | Responsibility |
| --- | --- |
| `lib.rs`, `main.rs` | Native commands/setup, application state and early MCP helper entry points |
| `turns.rs`, `router.rs`, `classify.rs` | Direct/Auto/review/Both lifecycle, routing and bounded classification |
| `native_claude.rs` | Bidirectional native Claude initialize/control/input/output stream |
| `approvals_codex.rs` | Codex app-server, thread start/resume, questions and native events |
| `approvals.rs`, `interactions.rs`, `activity.rs` | Scoped approvals/questions and bounded activity/child streams |
| `providers.rs`, `catalog.rs`, `proc.rs` | CLI launch, isolation, metadata, normalization and process-tree cancellation |
| `orchestra.rs`, `scheduling.rs` | Task graph, worker worktrees, caps, integration and recovery |
| `browser.rs`, `ollama_agent.rs`, `ollama.rs` | Shared Moli broker/MCP proxy, local API tool loop, model discovery/helpers |
| `store.rs`, `migrations.rs`, `workstate.rs` | SQLite persistence, atomic schema upgrades and durable decisions |
| `context.rs`, `codegraph.rs`, `graph_mcp.rs`, `impact.rs` | Context, pattern-based graph tools and pre/post-edit impact |
| `diagnostics.rs`, `validation.rs`, `projecttools.rs` | Bounded LSP/check execution and affected-project verification |
| `gitops.rs`, `merge.rs`, `workspace.rs` | Private-index checkpoints, immutable manual merges and explicit cwd changes |
| `inspector.rs`, `media.rs`, `review.rs`, `layout.rs`, `variant_preview.rs` | Local preview, images/data, source diffs, layout constraints and static variants |
| `godot.rs`, `screens.rs`, `hotkeys.rs` | Opt-in editor controls, capture/import and capture shortcut |
| `settings.rs`, `tray.rs`, `usage.rs`, `limits.rs` | Preferences, background continuity and observed provider availability |

Rust modules are under `src-tauri/src/`. Frontend components live under `src/components/`, settings under `src/settings/`, shared types/IPC under `src/lib/`. The native Gatekeeper crate and React review surface live in `src/features/gatekeeper/`; local-helper code is in `src/features/local-assistant/`.

## Turn lifecycle

1. Validate session, actual folder, model/reasoning and permissions; persist the request.
2. Acquire a job slot and, for writes, the canonical-workspace lock. Extra work remains cancellable.
3. Build direct/native continuation or the necessary bounded replay. Resolve Auto/Orchestra only when selected.
4. Register the invocation's Moli capability, context and environment. Launch the native CLI or official local Ollama API loop.
5. Normalize text, exposed summaries, questions, child agents, browser activity and images. Coalesce text deltas at 24 ms, flushing before control events.
6. Persist checkpoints and complete optional review/integration/validation stages. Record actual completion/error/interrupted state and release resources.

Native Claude/Codex continuations send the new request and changed context. Missing native sessions/fallback use replay; saved requirements are separate from transcript truncation. Provider authentication remains native. Prompts are written to stdin, not assembled as shell command strings.

## Browser boundary

`browser.rs` binds an invocation-local authenticated loopback endpoint. The same executable's `--lumen-browser-mcp` mode proxies stdio MCP before GUI startup. Capabilities pass through environment variables, not persisted project settings or prompts. Google account MCP entries are stable; actual invocation context remains per process.

One-shot Moli fetches avoid layout/images/fonts. Interactive CDP engines are retained per conversation/worker/workspace and start lazily. Inputs, output, concurrency and deadlines are bounded. Permission enforcement is inside the broker. Original screenshots are saved in the conversation project and delivered to the UI; Antigravity's observed inline-image issue uses its native file/image path instead.

Ollama requests stay on loopback with proxies/redirects disabled. `/api/show` selects native tool calling or a JSON-action format; tool results return to the same model. Remote model metadata/cloud aliases are rejected. [Detailed browser contract](moli.md).

## Persistence and ownership

The application identifier is `io.github.andrea17982.lumen`. `lumen.db` uses WAL/foreign keys and versioned transactional migrations. It stores sessions, messages/activity, accounts, project containers and task state; account records contain metadata rather than token contents. Native account directories are under `<app-data>/accounts/<UUID>`.

Projects and actual folders remain independent. Historical messages/captures keep their own root. Checkpoints and merge operations use a private Git index and preserve the user's staging. Shared skills/MCP definitions live in app data; project-owned instructions, checks and extensions remain explicit trust inputs.

A conversation's complete loaded transcript remains in memory, with bounded activity and historical Markdown/content-visibility optimizations. Full virtualization/warm process pools are not claimed. Work continues while the window is hidden to the tray; normal Quit requests cancellation. Crash/restart marks work interrupted and does not replay it automatically. Forced termination can leave descendants; recovery checks identity before stopping an old worker.

## UI contract and limits

`lumen:run` events carry session/message identities. Questions are separate from approvals. Secret answers do not enter public durable decision text. Markdown excludes raw HTML; remote images require user loading, local image paths are project-scoped, and links use validated backend open commands.

The inspector's static preview server owns only explicit built outputs and does not execute project commands. JSON/SQLite exploration is read-only. The layout canvas writes the project constraint spec. No general OpenAI-compatible inference server is implemented in this release.

[Security](../SECURITY.md) · [Validation](validation.md) · [Development](development.md)
