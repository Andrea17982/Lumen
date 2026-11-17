# Recoverable autonomy

Lumen lets assistants work on their own while keeping their work recoverable. It sorts every action into one of three classes, records them in an audit log for each conversation, and can roll the project back to a checkpoint. This page also lists what a rollback cannot undo.

## Action classes

| Class | Examples | What Lumen does |
| --- | --- | --- |
| **Reversible** | Reading and listing files, editing project files, builds and tests | Runs it under the chosen permission profile. A checkpoint restore undoes it. |
| **Risky / irreversible** | `rm -rf`, `rmdir /s`, `Remove-Item -Recurse`, `git reset --hard`, `git clean -f`, `git push --force`, branch/stash deletion, `DROP TABLE`, `DELETE FROM`, `terraform destroy` | Lumen's own tool loops (official APIs and local Ollama) **always** ask for one explicit answer, **even in Autopilot**. Ask first asks for every action. |
| **External** | `git push`, `npm publish`, `gh pr create`, `curl -X POST`, `ssh`, `kubectl`, `aws`, database clients (`psql`, `mysql`, `mongosh`, `redis-cli`, `sqlcmd`), browser click/fill/evaluate, third-party MCP tools | Runs it under the profile and records it as **not undone by rollback**. |

Lumen classifies actions in `lumen-core/src/autonomy.rs` by matching command words and tool names, conservatively and in a documented way. This classification is **not a sandbox**. An unusual spelling, a script that runs a destructive command internally, or an alias can still get through. Native CLIs (Claude Code, Codex, Antigravity, OpenCode, Cursor) in Autopilot run their own tools: Lumen classifies and logs those actions from the turn's activity but does not block them beforehand. Use **Ask first** or **File edits** when an action must be stopped before it runs.

## Audit log

The `audit_log` command returns the classified actions of a conversation, oldest first. It covers shell commands, tool calls, file changes, browser actions, and Lumen's own decisions: *confirmed*, *denied*, *ran* for external actions, and *rollback*. Every provider records activity in a different format, and Lumen reads all of them. Rollbacks are also appended and synced to `audit/<conversation>.jsonl` in the data directory, so they remain in the log after Rewind removes the messages that followed.

## Checkpoints and rollback

Before each turn that can write, Lumen takes a snapshot of the project through a private Git index. The snapshot includes tracked and untracked files and respects `.gitignore`. The user's staging area, HEAD and branches are never touched. Rewind previews the files a restore would change, then restores them and truncates the conversation.

Since 0.12:

- **Byte-exact snapshots.** Files with `text`, `eol`, `filter`, `ident` or `working-tree-encoding` attributes are stored and restored exactly as they were on disk, so CRLF, mixed line endings and `core.autocrlf` cannot change a restored file. Filter drivers, Git LFS included, are turned into no-ops during snapshot and restore, so no external process or download runs, even when the driver is marked required.
- **Undoable restore** (adopted from Hermes Agent's rollback). Before restoring, Lumen snapshots the state it is about to replace under `refs/lumen/checkpoints/before-restore-*`.
- **Restore safety** (adapted from T3 Code's `CheckpointRestoreSafety`). A restore rewrites the whole Git checkout. Lumen refuses it while another conversation whose folder **overlaps** that checkout (the same folder, a parent or a subfolder) is running, or when such a conversation has changed since the chosen message.
- **Provider consistency.** Lumen refuses Rewind when a later turn's native CLI session ran in a folder outside the restored checkout, because files and conversation could not then be rewound together. After a rewind, native resume markers are cleared, so the next turn replays the retained history instead of continuing a provider conversation that still contains the discarded future.
- Fixtures cover CRLF/eol attributes, a failing required filter, six concurrent checkpoints and the user's index, and a restore that is exact (the tree after restore equals the checkpoint tree) and can be undone.

## Untrusted content

Moli pages, web fetches and searches, third-party MCP tools, connectors and commands that reach the network are treated as **data, never instructions**. If a reply proposes project memory (`<lumen-remember>`) after the turn read any of these, Lumen does **not** save it to `LUMEN.md` automatically. The fact appears under the reply with a **Save** button, and the `remember_project` command saves only a fact that was held this way. A recurring task proposed in such a reply is not scheduled. Global memory always needs confirmation.

## Limits and boundaries

Rollback restores **only the files of the project's Git checkout** that are not ignored. It does **not** undo:

- **Databases.** Local files listed in `.gitignore`, such as SQLite or `.env`, and any database server are not restored. Migrations, seeds and SQL commands are permanent.
- **Remote services.** Pushes, publications, deployments, pull requests and issues, API calls, emails and cloud resources stay as they are.
- **Files outside the project**: home-directory configuration, global package caches, other repositories, the system.
- **Ignored files** (`node_modules`, build output, `.env`) and folders that are not Git repositories, which have no checkpoint.
- **Running processes** started by a command, and the state of a browser page.

Limits on time, turns, resources and API spend are covered by Orchestra's per-run limits and by the [API budget reservations](api-access.md). Reservations survive a crash and still count after a restart, so stopping and resuming never resets a limit. The IPC boundary accepts only the commands in the core dispatch table. The local service authenticates its peer ([Local service](local-service.md)). The Moli broker enforces the access profile even when a CLI auto-approves MCP calls ([Moli](moli.md)).
