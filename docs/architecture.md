# Architecture

The local daemon owns everything: storage, accounts, process and API execution, permissions, routing and cancellation. Clients are thin. They render state and send commands over authenticated local IPC. The desktop client is being rewritten.

## Crates

| Crate | Role |
| --- | --- |
| `lumen-core` | All domain logic (modules below). |
| `lumen-daemon` | Service entry point. Also `--lumen-code-mcp`, `--lumen-browser-mcp` and `--lumen-plugin-mcp` helper modes. |
| `lumen-ipc` | Local transport, peer authentication, protocol and build checks. |
| `lumen-commands` | Typed command boundary. |
| `lumen-gatekeeper` | Native approval broker for Claude and Codex, with protocol fixtures. |

## `lumen-core` modules

| Module | Responsibility |
| --- | --- |
| `turns.rs`, `router.rs`, `classify.rs` | Direct/Auto/review lifecycle, routing, classification |
| `native_claude.rs`, `approvals_codex.rs` | Native Claude stream; Codex app-server |
| `opencode.rs`, `opencode_acp.rs`, `cursor.rs`, `droid.rs` | OpenCode catalogs and ACP; Cursor and Factory Droid headless runners |
| `api.rs`, `api_wire.rs`, `api_tools.rs` | Keychain and reservations; three wire formats; host file/command tools |
| `approvals.rs`, `interactions.rs`, `activity.rs` | Scoped approvals, questions, activity and child streams |
| `capability_catalog.rs`, `integrations.rs`, `native_mcp.rs`, `shared_tools.rs` | Adapter contract, integration discovery, account gateway, shared MCP client |
| `providers.rs`, `catalog.rs`, `proc.rs` | CLI launch, isolation, metadata, process-tree cancellation |
| `orchestra.rs`, `scheduling.rs` | Task graph, worker worktrees, caps, integration, recovery |
| `browser.rs`, `ollama.rs`, `ollama_agent.rs` | Moli broker and MCP proxy; Ollama discovery and tool loop |
| `store.rs`, `migrations.rs`, `workstate.rs` | SQLite, migrations, durable task ledger |
| `context.rs`, `codegraph.rs`, `graph_mcp.rs`, `impact.rs` | Context, pattern graph, pre/post-edit impact |
| `diagnostics.rs`, `validation.rs`, `projecttools.rs` | LSP and project checks |
| `autonomy.rs` | Action classes, audit log, restore safety, untrusted-content gating |
| `gitops.rs`, `workspace.rs`, `review.rs` | Private-index checkpoints, worktrees and Publish, diffs |
| `execution.rs` | Content-free timing and phase snapshots |
| `settings.rs`, `host.rs`, `usage.rs`, `limits.rs` | Preferences, event journal, quota observation |

## Turn lifecycle

1. Validate the session, folder, model and permissions, then persist the request.
2. Acquire a job slot and, for writes, the canonical-workspace lock.
3. Build a native continuation, or the bounded replay that is actually needed. Resolve Auto or Orchestra only when selected.
4. Register the invocation's Moli capability and environment, then launch the CLI, the API loop or the Ollama loop. Prompts go to stdin and are never assembled into shell strings.
5. Normalize text, questions, child agents, browser activity and images. Text deltas are coalesced at 24 ms.
6. Persist checkpoints, run any review, integration or validation, then record the final state. A provider "done" does not certify the task.

Replay guard: a missing native session or a quota error may fall back once, and only before any observed work. Tools, thinking, input and unknown activity all block replay.

Native continuations send only the new request plus the Lumen context that changed: project notes, and check evidence for the current chat and workspace. A switch to another account uses a full bounded handoff and never reuses another account's session ID.

## Persistence

- The app identifier is `io.github.andrea17982.lumen`.
- `lumen.db` (SQLite, WAL) is opened **only by the daemon**, under one OS file lock per data directory. It holds sessions, messages and activity, account metadata (never tokens), projects and task state.
- Schema 5 adds command receipts. Older schemas upgrade in place, and a newer schema is refused. Back up the data directory before upgrading.
- Run events reach SQLite before any client. Snapshots carry sequence numbers, so clients can deduplicate streamed events against them.
- Tool output over 16 KiB reaches clients truncated, with a `detail` digest. `activity_detail(id, message, digest)` fetches the full text. The store keeps tool output and diffs up to 1 MiB per step; longer output is cut once, with a visible marker, before it is stored or sent. One message keeps up to 8 MiB of such output; later steps in it keep their first 16,000 characters.
- Projects are containers that do not depend on folders. Checkpoints use a private Git index.

## Command receipts

Mutating commands carry a UUID: `create_session`, `send_message`, `cancel_run`, `answer_question`, `gatekeeper_decide`, `resume_at_reset`, and Orchestra answer, approve, cancel, resume and raise.

- The daemon commits a claim before dispatch and the reply before delivery.
- After a transport failure a client retries once with the same UUID. The same UUID with different arguments is rejected.
- An interrupted claim is left uncertain and never replayed. This is not exactly-once execution for opaque tools.

## Crash, quota and process ownership

- A client crash does not stop the daemon.
- A daemon crash or reboot marks unfinished work as interrupted. Partial output stays in the journal, and the work needs an explicit Resume. Nothing is resubmitted automatically.
- Scheduled quota continuations survive restarts. They are revalidated, and consumed before being submitted.
- On Windows, a kill-on-close Job Object owns all descendants. On Unix, each command gets a private process group with a supervisor tied to the daemon's lifetime.
- Cleanup never searches processes by name or trusts a stale PID. An ambiguous live process blocks Resume until it is reviewed.

## Local security boundary

- On Windows the transport is a named pipe. Remote clients are rejected, and client and server compare process-token SIDs.
- On Unix it is a private socket directory with peer UID checks.
- The handshake checks the protocol version and the daemon binary's SHA-256. This is a compatibility check, not protection against a malicious process running as the same user.
- There is no TCP listener. Invocation-scoped loopback MCP brokers (Moli, Code Mode) use random capabilities passed through environment variables, never through prompts or files.

## Daemon CLI

```powershell
.\lumen-daemon.exe --status
.\lumen-daemon.exe --prepare-update   # refuses connected clients or active/scheduled work
.\lumen-daemon.exe --shutdown         # cancels active work; waiting quota intentions survive
```

`--data-dir <dir>` sets a separate data root and `--resources <dir>` an explicit resource root. Never overwrite the daemon binary while it is running. Packaging follows Tauri's [sidecar contract](https://v2.tauri.app/develop/sidecar/).

The local-service parts are adapted from T3 Code; see [credits](../src-tauri/resources/legal/t3code/ADAPTATIONS.md).
