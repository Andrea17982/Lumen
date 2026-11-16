# Architecture

Tauri owns the native window and desktop actions. A separate local daemon owns storage, account configuration, process/API execution, permissions, routing and cancellation through `lumen-core`. The GUI is a client of authenticated local IPC. React renders normalized events. The webview does not read provider credentials or contact inference services directly.

OpenCode and Cursor use one existing native profile each and remain explicitly selected. `opencode.rs` discovers project-scoped models; `opencode_acp.rs` creates or verifies native session policies before prompting, bridges official ACP approvals/events, and injects scoped Moli/graph/MCP servers. `cursor.rs` retains the headless runner and successful-terminal-result requirement; Cursor is Autopilot only and has no shared-tool bridge. Owned process trees are cancelled without touching unrelated CLIs.

`api.rs` owns OS-keychain lookup, official HTTPS requests and transactional reservations in `api.sqlite`; `api_wire.rs` preserves the three official tool protocols; `api_tools.rs` enforces host file/command permissions. The frontend receives keySaved booleans, never saved keys. Every paid tool-loop step rechecks budget and Auto opt-in. API accounts are excluded from Orchestra workers until its later budgeted collaboration milestone; direct Auto/fallback requires explicit API opt-in. See [API limits](api-access.md).

## Execution modules

| Module | Responsibility |
| --- | --- |
| `lumen-core/lib.rs`, `lumen-daemon/main.rs` | Domain commands/state and headless service/MCP helper entry points |
| `turns.rs`, `router.rs`, `classify.rs` | Direct/Auto/review/Both lifecycle, routing and bounded classification |
| `native_claude.rs` | Bidirectional native Claude initialize/control/input/output stream |
| `approvals_codex.rs` | Codex app-server, thread start/resume, questions and native events |
| `approvals.rs`, `interactions.rs`, `activity.rs` | Scoped approvals/questions and bounded activity/child streams |
| `capability_catalog.rs`, `integrations.rs` | Read-through integration discovery, original skill resources, per-source namespaces and per-invocation client configuration |
| `native_mcp.rs` | Direct native-account tool/skill RPC gateway, with turn-scoped host consent and no model turn |
| `execution.rs` | Bounded content-free invocation observations, phase/timing snapshots and internal-stage handoff |
| `providers.rs`, `catalog.rs`, `proc.rs` | CLI launch, isolation, metadata, normalization and process-tree cancellation |
| `orchestra.rs`, `scheduling.rs` | Task graph, worker worktrees, caps, integration and recovery |
| `browser.rs`, `ollama_agent.rs`, `ollama.rs` | Shared Moli broker/MCP proxy, local API tool loop, model discovery/helpers |
| `shared_tools.rs` | Turn-owned stdio MCP client for Ollama, scoped shared skills, host consent and process cleanup |
| `store.rs`, `migrations.rs`, `workstate.rs` | SQLite persistence, atomic schema upgrades and durable decisions |
| `context.rs`, `codegraph.rs`, `graph_mcp.rs`, `impact.rs` | Context, pattern-based graph tools and pre/post-edit impact |
| `diagnostics.rs`, `validation.rs`, `projecttools.rs` | Bounded LSP/check execution and affected-project verification |
| `gitops.rs`, `merge.rs`, `workspace.rs` | Private-index checkpoints, immutable manual merges and explicit cwd changes |
| `inspector.rs`, `media.rs`, `review.rs`, `layout.rs`, `variant_preview.rs` | Local preview, images/data, source diffs, layout constraints and static variants |
| `godot.rs`, `screens.rs` | Validated editor targets and capture/import; desktop launch/hiding/shortcut remain in the GUI |
| `settings.rs`, `host.rs`, `usage.rs`, `limits.rs` | Preferences, event journal/login startup and observed provider availability |

Domain modules are under `src-tauri/crates/lumen-core/src/`; `lumen-daemon` is the service entry point, `lumen-ipc` owns local transport/authentication and `lumen-commands` generates typed JSON boundaries. Only desktop setup, tray, hotkeys and launch actions remain under `src-tauri/src/`. Frontend components live under `src/components/`, settings under `src/settings/`, shared types/IPC under `src/lib/`. The native Gatekeeper crate and React review surface live in `src/features/gatekeeper/`; local-helper code is in `src/features/local-assistant/`.

## Turn lifecycle

1. Validate session, actual folder, model/reasoning and permissions; persist the request.
2. Acquire a job slot and, for writes, the canonical-workspace lock. Extra work remains cancellable.
3. Build direct/native continuation or the necessary bounded replay. Resolve Auto/Orchestra only when selected.
4. Register the invocation's Moli capability, context and environment. Launch the native CLI, official API tool loop or local Ollama API loop.
5. Normalize text, exposed summaries, questions, child agents, browser activity and images. Coalesce text deltas at 24 ms, flushing before control events.
6. Persist checkpoints and complete optional review/integration/validation stages. Record the turn's return/error/interrupted state and release resources; a provider return does not certify completion of the requested task.

Native Claude/Codex continuations send the new request and changed context. A missing native session can fall back once to full replay only before observed work, with no partial text or cancellation. The guard sees lifecycle events before compaction; tools, thinking, input and unknown activity prevent replay. This guard is distinct from other fallback policies. Saved requirements are separate from transcript truncation. Provider authentication remains native. Prompts are written to stdin, not assembled as shell command strings.

Lumen fingerprints Lumen-owned project notes and current-chat/current-workspace check evidence. Native continuation sends this context only when changed; account handoffs include it in the full bounded prompt without crossing native account identities. Auto's quota selection is not overwritten by previous-account affinity. Explicit audit intent survives classifier output; a separate primary account, preferably another provider, performs read-only review. Both retains its Claude-plus-ChatGPT contract. See [task quality](task-quality.md) for bounds and [validation](validation.md) for evidence.

Lumen records versioned `execution` snapshots and `execution-phase` markers in existing message activity. They contain bounded timestamps/counts/phases, not prompt, response or command bodies. `stage_with` observes provider lifecycle, persists visible phase changes, and publishes other snapshots at most every 750 ms plus the final state. Internal Orchestra scratch stages explicitly merge execution snapshots into durable output. Native tool start/end identifiers update aggregate state rather than accumulating hidden per-tool rows. The selected-turn UI consumes snapshots, native input/approval state and structured validation. Read-only status refresh is guarded against newer streaming, approval events and chat switches. No silence-triggered retry is implemented. Pre-acceptance frontend waiting is separate from backend invocation observations.

Quota fallback shares the missing-session activity guard. It still cools the limited account, but preserves partial output and does not switch/replay after observed work or cancellation. A pre-work quota failure can switch once. The guard is shared through a nested missing-session replay, so later work cannot be forgotten by the outer quota handler.

## Browser boundary

`browser.rs` binds an invocation-local authenticated loopback endpoint. The same executable's `--lumen-browser-mcp` mode proxies stdio MCP before service startup. Capabilities pass through environment variables, not persisted project settings or prompts. Google account MCP entries are stable; actual invocation context remains per process.

One-shot Moli fetches avoid layout/images/fonts. Interactive CDP engines are retained per conversation/worker/workspace and start lazily. Inputs, output, concurrency and deadlines are bounded. Permission enforcement is inside the broker. Original screenshots are saved in the conversation project and delivered to the UI; Antigravity's observed inline-image issue uses its native file/image path instead.

Ollama requests stay on loopback with proxies/redirects disabled. `/api/show` selects native tool calling or a JSON-action format; tool results return to the same model. Remote model metadata/cloud aliases are rejected. [Detailed browser contract](moli.md).

## Graph reliability

The graph corrects scoped typed-module extension substitution and Godot literal resource/inheritance paths. Full-line TS/JS/C#/GDScript comments are excluded from extraction/reference identifiers. Relative normalization stays within indexed repository/Godot-project boundaries; index format 4 invalidates earlier extraction records. Existing immutable pre-edit graph snapshots retain callers after deletions. This remains an approximate graph, not a complete compiler/configuration resolver.

## Turn timing

Windows metadata CLI calls now start inside a kill-on-close Job Object while suspended, then resume without a console window. EOF gets at most 80 ms grace before remaining members are terminated/reaped; cancellation/errors skip grace, task abortion drops the job, and exited roots cannot orphan their normal Win32 descendants. Assignment failure does not retry unsupervised. Model-turn and non-Windows termination are unchanged.

Lumen adds bounded monotonic offsets to existing execution snapshots and `preparationMs` to accepted direct submissions. Transport markers are source-stamped before event queues; normalized first event/text remains a controller observation. Timing control signals do not add per-marker IPC/SQLite writes or include prompt/path/account content. Installed CLI metadata probes are opt-in, isolated and inference-free; actual provider TTFT and worker scheduling remain separate.

## Persistence and ownership

The application identifier is `io.github.andrea17982.lumen`. `lumen.db` uses WAL/foreign keys and versioned transactional migrations. It stores sessions, messages/activity, accounts, project containers and task state; account records contain metadata rather than token contents. Native account directories are under `<app-data>/accounts/<UUID>`.

Projects and actual folders remain independent. Historical messages/captures keep their own root. Checkpoints and merge operations use a private Git index and preserve the user's staging. Shared skills/MCP definitions live in app data; project-owned instructions, checks and extensions remain explicit trust inputs.

A conversation's complete loaded transcript remains in memory, with bounded activity and historical Markdown/content-visibility optimizations. Full virtualization/warm process pools are not claimed. Work continues while the window is hidden to the tray; normal Quit requests cancellation. Crash/restart marks work interrupted and does not replay it automatically. Forced termination can leave descendants; recovery checks identity before stopping an old worker.

Lumen isolates the full transcript in memoized `Transcript`, reuses unchanged message/token projections and skips unrelated execution-summary parent updates. Stable callback identities forward to the latest committed chat and permission state; merge readback is selected-chat guarded. Real transcript changes still render MessageView normally. No virtual window, new history limit or CSS/scroll policy is introduced. Windows typing measurements retain nonstationary absolute timings. A subsequent opening experiment accepted an exact literal-paragraph path in MessageBody: Markdown and ambiguous input retain ReactMarkdown/GFM. All history remains mounted. A variable-height virtual-window test fixture fails content-access gates and is compiled only with `VITE_TRANSCRIPT_EXPERIMENT=1`; ordinary builds remove its import and ignore the experiment query parameter.

## UI contract and limits

`lumen:run` events carry session/message identities. Questions are separate from approvals. Secret answers do not enter public durable decision text. Markdown excludes raw HTML; remote images require user loading, local image paths are project-scoped, and links use validated backend open commands.

The inspector's static preview server owns only explicit built outputs and does not execute project commands. JSON/SQLite exploration is read-only. The layout canvas writes the project constraint spec. No general OpenAI-compatible inference server is implemented in this release.

[Security](../SECURITY.md) · [Validation](validation.md) · [Development](development.md)

## Local service boundary

[Service operations](local-service.md) describes protocol/build checks, OS peer authentication, exclusive ownership, schema-4 event/checkpoint recovery, durable quota intentions, process containment and the verified stop-before-update procedure. The GUI never opens the domain database or owns model processes; editor/login/link opening stays outside the service process tree. No remote listener is added.
