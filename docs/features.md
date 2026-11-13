# Features and working modes — 0.8

This guide describes the current product. Historical changes belong in the [release notes](release-notes/README.md).

## Direct native sessions

Select a connected provider and an available model. Direct mode is the default and preserves the explicit provider/model choice; classification does not silently switch a manually selected architecture or security request to another provider. Expand the composer controls for account, model and supported reasoning levels.

Claude uses a bidirectional native control stream; Codex uses app-server in normal and Orchestra turns. They support native continuation, exposed reasoning/activity, structured questions and native child-agent output. Questions can offer choices, multiple selections, free text or secret input. A question is not a permission grant. Google providers use their official print runners; Ollama uses its official loopback API. These transports do not have identical interactive capabilities.

## Auto

Auto optionally classifies and routes the task using connected catalogs, with keyword fallback and reuse of a short follow-up's earlier classification. Budget/Default/Quality adjust effort and independent review. Additional stages consume quota. [Routing and quota rules](auto.md).

## Orchestra

Enable Orchestra explicitly. It can coordinate complex Auto requests and approved multi-part work, with task specifications, dependencies, file ownership and editable agent assignments. It retains the selected coordinator model where applicable.

1. Ask only clarifications that materially affect the result.
2. Prepare a task graph and agent assignments. Approval-requiring profiles can review it; Autopilot or already approved execution continues without another plan gate.
3. Run bounded workers in isolated Git worktrees, respecting dependencies and concurrency/total-subtask caps. Queue and worker output are visible.
4. Integrate results and run the combined validation/final check. Failures are reported; retained branches remain available for recovery where integration could not finish.

Workers receive the team manifest, relevant prior decisions, impact/contracts and the real execution folder. Writing work requires Git. Read-only investigations remain read-only. Supported native approval/question connections reach workers instead of being replaced by extra planning steps.

Closing to the tray keeps work running. After a crash/restart, unfinished work is interrupted and requires explicit Resume; a turn is not silently replayed. [Recovery and limitations](validation.md).

## Both and manual merge

Both compares Claude and Codex side by side in the read-only interface. Worktree-backed variants support changes inspection, per-file selections and manual merge text where applicable. Merge previews use immutable Git snapshots and a private index, preserve unrelated staging and reject stale source previews. Retained static web outputs can be compared in the inspector without executing arbitrary project build commands.

## Context and project location

Projects group conversations independently of their working folders. `/workspace <absolute path>` changes the selected conversation's folder explicitly; ambiguous references or quoted instructions do not move it. `/compact` and the context popover support a fresh continuation with retained decisions. Folder changes are refused while work is active.

The durable ledger records source-linked requirements, approved scope, actual permissions/workspace, results and verification. Native resume sends the new request plus changed context; full replay is used only where needed. Local token estimates use a tokenizer and remain distinct from provider reports. Large logs can spill to project-local files, and pasted lockfiles are blocked. [Task continuity](task-quality.md).

## Browser and images

Moli is bundled and automatically wired into every provider and worker invocation. It supports web retrieval/search, persistent navigation, extraction, click/fill/evaluation and screenshots. It starts on demand and isolates conversation/worker contexts. Read only/File edits reject page mutations; Autopilot proceeds; Ask-first browser mutations get a one-action decision.

Local captures are shown with image enlargement and keyboard focus restoration. Remote response images load only after the user requests them. Antigravity uses saved PNG paths for its native viewer instead of the inline MCP image handoff that stalled in live tests. [Moli details](moli.md).

## Inspector and Godot

The inspector offers local web preview, image pixel zoom/grid, read-only JSON/SQLite, project checks, changes, scene/resource diffs, screenshot comparison, layout blocks and retained static worktree variants. Layout constraints are saved per project and delivered as context changes to native sessions.

Godot discovery and Play/Stop/Reload/viewport capture are opt-in and verify the connected editor root. Native screen/window capture and image import are also available; an optional global hotkey attaches a capture. Registration itself never captures. Purchased addons and runtime services are not enabled or redistributed implicitly.

## Checks, review and Rewind

Additional Lumen validation is opt-in for ordinary native sessions. When enabled, affected diagnostics/checks provide concrete failures for bounded corrections; Orchestra retains final validation. Missing coverage is explicit. Independent cross-audit is optional and uses another configured provider.

Rewind snapshots tracked working-tree state through a private Git index, previews restoration and rebuilds retained task state. Ignored files are not restored. File writes into the same canonical folder are serialized across Lumen conversations.

## Local helper and settings

The optional Ollama helper generates titles, editable commit-message drafts and handoff summaries. It never commits or sends work to a remote inference provider. A helper failure preserves manual editing. Its opt-in settings are separate from the selected Ollama conversation model.

Simple settings expose the Orchestra switch; Advanced reveals the other sections and controls. Restore defaults preserves accounts, conversations and project folders. Appearance supports English/Italian, light/dark/system themes and a larger-text preference. See [Security](../SECURITY.md) for full-access and storage boundaries.
