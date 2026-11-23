# Features and working modes

This guide describes the current product. Historical changes belong in the [release notes](release-notes/README.md).

## Direct native sessions

Select a connected provider and an available model. Auto is the initial mode; choosing a provider preserves the explicit provider/model choice; classification does not silently switch a manually selected architecture or security request to another provider. Expand the composer controls for account, model and supported reasoning levels.

Claude uses a bidirectional native control stream; Codex uses app-server in normal and Orchestra turns. They support native continuation, exposed reasoning/activity, structured questions and native child-agent output. Questions can offer choices, multiple selections, free text or secret input. A question is not a permission grant. In Auto, Lumen answers plain clarifying questions itself and lists its assumptions in the reply ([Questions in Auto](auto.md#questions-in-auto)). Google providers use their official print runners; Ollama uses its official loopback API. These transports do not have identical interactive capabilities. OpenCode uses official ACP and a verified native session-policy API, with native continuation, one-shot approvals and shared Lumen tools. Cursor keeps explicit Autopilot selection and native headless continuation. The [three official API adapters](api-access.md) use OS-keychain keys, common tools, all four permission profiles and reserved cost budgets; their JSON responses are buffered.

## Auto

Auto optionally classifies and routes the task using connected catalogs, with keyword fallback and reuse of a short follow-up's earlier classification. Budget/Default/Quality adjust effort and independent review. Additional stages consume quota. [Routing and quota rules](auto.md).

## Orchestra

Enable Orchestra explicitly. It can coordinate complex Auto requests and approved multi-part work, with task specifications, dependencies, file ownership and editable agent assignments. It retains the selected coordinator model where applicable.

1. Ask only clarifications that materially affect the result.
2. Prepare a task graph and agent assignments. Approval-requiring profiles can review it; Autopilot or already approved execution continues without another plan gate.
3. Run bounded workers in isolated Git worktrees, respecting dependencies and concurrency/total-subtask caps. Queue and worker output are visible.
4. Integrate results and run the combined validation/final check. Failures are reported; retained branches remain available for recovery where integration could not finish.

Workers receive the team manifest, relevant prior decisions, impact/contracts and the real execution folder. Writing work requires Git. Read-only investigations remain read-only. Supported native approval/question connections reach workers instead of being replaced by extra planning steps.

**Parallel or Collaborative.** After planning, Lumen picks the team shape with a fixed rule. Collaborative applies when the steps cannot really run side by side: every step needs all earlier ones, or two writing steps claim the same file. The agents then share one plan and take turns in plan order, alternating Claude and ChatGPT where both are available. Each turn starts from the previous turns' changes and reads their results. Otherwise the run is Parallel. Both shapes use the same time, concurrency, total-subtask and quota limits; a Collaborative run simply never has two turns running at once. The run ends with **Team result by Lumen**, which states the mode.

**Compete.** Only in the Quality profile, because it uses about twice the quota: for one important or uncertain change, when two different providers are available within quota, Auto has both build it independently in separate worktrees. Lumen runs the project's checks on each, a reviewer from another provider compares the passing ones, and Lumen applies the winner. The team result says which candidate won, why, and what failed. Otherwise Auto runs Single (with Review when enabled) and states why. [Rules and measured results](auto.md#compete).

**Worker approval inbox.** Approval requests and native questions from a worker show which agent asked and for which subtask, next to the reason the CLI gave. Parallel workers in one chat keep separate approval connections. When the worker's process ends, its open request becomes expired: it stays visible with a note that the worker stopped, and it can no longer be approved. This is the existing Gatekeeper and question flow, not a second permission system.

Closing the GUI keeps work running in the local service, including native questions and Orchestra. After a service crash/restart, unfinished execution requires explicit Resume; a turn is not silently replayed. Explicitly scheduled quota intentions persist and are revalidated separately. [Service lifecycle](local-service.md). [Recovery and limitations](validation.md).

## Task worktree and Publish

A large Auto task that writes files (rated complex or top tier) runs in the conversation's own Git worktree, so the project folder stays as it was. Small tasks, Read only conversations and Orchestra runs keep working in the folder.

- **Ready to work.** The branch is named after the request (`lumen/<first words>-<id>`). Uncommitted changes in the folder become the branch's first commit. Ignored local files that a fresh checkout needs (`.env*` and `*.local*`, up to 1 MB each) are copied. In Autopilot, Lumen installs dependencies from the lockfile (npm, pnpm, yarn, bun or uv). In other profiles the agent installs them only if it needs them.
- **Commit after checks.** After each turn that finishes without failing checks, Lumen commits the changes on the branch. The message is drafted by a short Claude call, with a plain fallback, and ends with `Generated-by: Lumen`. Follow-ups continue in the same worktree.
- **Publish, Apply or Discard.** The topbar shows the branch, and three actions sit above the composer.
  - **Publish** first shows the remote, base and branch, with an editable title and description. It then pushes the branch and opens a ready pull request with `gh`. Publishing twice reuses the open pull request.
  - **Apply to folder** copies the task's changes into the project folder.
  - **Discard** deletes the worktree and its branch.

  After Apply or Discard, the conversation returns to the project folder.
- **Corrections after Publish.** The worktree stays, and Lumen's service checks the pull request every minute.
  - **When it sends a correction:** when a check fails or someone with write access to the repository requests changes. Lumen then sends this conversation a correction request with the failed CI log or the review comments, presented as data to fix rather than instructions. The fix is committed after Lumen's checks and pushed to the same branch.
  - **When it stops:** after three correction rounds, or when a correction ends without a commit, Lumen stops and hands over. You can also stop it with **Stop following**.
  - **When it merges:** once every check has finished green and no trusted reviewer's latest review still requests changes, Lumen merges the pull request. If the base branch has protection rules that are not met yet, such as a required review, Lumen turns on GitHub's automatic merge instead, which waits for them. Lumen merges only while it is running.
  - **When the pull request is merged:** the conversation returns to the project folder and the worktree is removed.

Publish needs an `origin` remote and a signed-in GitHub CLI; without them the button says why.

## Context and project location

Projects group conversations independently of their working folders. `/workspace <absolute path>` changes the selected conversation's folder explicitly; ambiguous references or quoted instructions do not move it. `/compact` and the context popover support a fresh continuation with retained decisions. Folder changes are refused while work is active.

The durable ledger records source-linked requirements, approved scope, actual permissions/workspace, results and verification. Native resume sends the new request plus changed context; full replay is used only where needed. Local token estimates use a tokenizer and remain distinct from provider reports. Large logs can spill to project-local files, and pasted lockfiles are blocked. [Task continuity](task-quality.md).

## Memory, recipes and recurring tasks

Assistants save durable project facts to `LUMEN.md`, dated and with their source chat. Preferences for all your projects are only proposed; click **Save** under the reply to keep them in global memory. The bookmark under a finished reply saves it as a recipe any assistant can repeat. Ask for repeated work in plain language or with `/every` (`/every 1d …`, `/every 08:00 …`, `/every lunedì 09:00 …`); the local service runs it in the same chat until you stop it with × above the composer or its end date passes. [Details](task-quality.md#project-memory).

## Browser and images

Moli is bundled and automatically wired into every provider and worker invocation. It supports web retrieval/search, persistent navigation, extraction, click/fill/evaluation and screenshots. It starts on demand and isolates conversation/worker contexts. Read only/File edits reject page mutations; Autopilot proceeds; Ask-first browser mutations get a one-action decision.

Local captures are shown with image enlargement and keyboard focus restoration. Remote response images load only after the user requests them. Antigravity uses saved PNG paths for its native viewer instead of the inline MCP image handoff that stalled in live tests. [Moli details](moli.md).

## Inspector

The inspector offers local web preview, image pixel zoom/grid, read-only JSON/SQLite, project checks, changes and scene/resource diffs. An optional global hotkey attaches a screen capture to the next message; registration itself never captures.

Godot support is a separate plugin in [plugins/godot](../plugins/godot/README.md): it finds Godot projects and the editor, opens the editor, and teaches agents to run and capture scenes through the project's own Godot MCP server.

## Checks, review and Rewind

Additional Lumen validation is opt-in for ordinary native sessions. When enabled, affected diagnostics/checks provide concrete failures for bounded corrections; Orchestra retains final validation. Missing coverage is explicit. Independent cross-audit is optional and uses another configured provider.

Rewind snapshots tracked and untracked working-tree state byte for byte through a private Git index, previews restoration and rebuilds retained task state. Ignored files are not restored. Rewind is refused while an overlapping conversation could be affected; see [Recoverable autonomy](autonomy.md) for action classes, the audit log and rollback limits. File writes into the same canonical folder are serialized across Lumen conversations.

## Settings

Simple settings expose the Orchestra switch; Advanced reveals the other sections and controls. Restore defaults preserves accounts, conversations and project folders. Appearance supports English/Italian, light/dark/system themes and system display scaling. See [Security](../SECURITY.md) for full-access and storage boundaries.
