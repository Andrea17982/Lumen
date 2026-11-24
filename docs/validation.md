# Validation and known limits

What has been verified for the current source (0.12.2), and what remains open. Fixtures, small real-account checks and observations are different kinds of evidence and are reported separately. Lumen is an alpha with no independent security audit. Evidence for earlier versions is in their Git tags.

## Automated

- About 293 Rust tests (17 opt-in ignored), Clippy with warnings denied, formatting, the daemon smoke test over IPC, and documentation and version checks. CI runs them on Windows, macOS and Linux.
- **Permissions.** A matrix checks every access profile against read, write, command, destructive command and `git push`, for both classic and Code Mode calls.
- **Auto.** Deterministic rule tests for Parallel, Collaborative and Compete.
- **Autonomy.** Fixtures for CRLF and Git filters, concurrent checkpoints, exact and undoable restore, Rewind refusals, and memory held after untrusted content.
- **Handoff.** Constraints and decisions survive two compactions on all core agents and APIs.
- **Plugins.** Manifest validation, refusal of undeclared capabilities, and re-approval when permissions grow.

## Unreleased: worktree and Publish

- **Rust end-to-end test** with a temporary repository, a bare remote and a fake `gh`. It covers branch creation, the commit of uncommitted work, copying `.env` but not `node_modules`, the automatic commit, the push, `gh pr create`, Apply and Discard.
- **Correction loop.** Failed logs and owner reviews reach the correction request, untrusted reviewers are ignored, green checks lead to the merge, and a correction without a commit stops the loop.
- **Live run** on a disposable private GitHub repository with Claude Code. Worktree, commit, Publish, green CI and merge all worked. The run found and fixed three bugs: a line-ending commit, a premature `gh pr merge --auto`, and a missing Claude session.

## Code Mode measurements

| Run | Result |
| --- | --- |
| Automatic, gpt-oss 20B local, 50 paired runs | 45 → 50 successes, none lost. Many-call edits: tokens −45%, round-trips −42%. [Results](../reports/bench/2026-11-23/summary.md) |
| Automatic, Claude Code (Haiku), 18 paired runs | 14 → 16 correct. Uncached tokens −56% overall, −81% on large reads. [Results](../reports/bench/2026-11-23/summary.md#t3--cli-in-automatico) |
| Codex | 18/18 → 14/18 with more tokens, so Codex keeps direct tools. |
| Always mode, Claude Code, large results | Uncached tokens −90%, answers 1/3 → 2/3. [Results](../reports/bench/2026-11-22/summary.md) |
| Qwen3-Coder 30B local | Gains on repeated edits, loses on log filtering. Never calls `llm()`. [Results](../reports/bench/2026-11-18/summary.md) |

Paid APIs have not been measured. No configuration passes the gate for making Code Mode always-on.

## Earlier live observations

These are single configurations, not certified:

- recurring-task and memory recognition with Claude Sonnet (6/6, 3/3);
- Codex native questions and continuation;
- a two-worker Orchestra run;
- Moli on Codex and Antigravity;
- Ollama tool calls;
- real language-server errors.

## Known limits

- **Platforms.** Only Windows x64 has had interactive desktop acceptance. macOS and Linux pass CI only.
- **Live coverage.** There is no live quality benchmark. The multi-account, CLI-version, quota and restart matrix is not certified.
- **Integrations.** Portability is limited to the documented subset ([Providers](providers.md#shared-integrations)). Cursor runs in Autopilot only. API adapters buffer responses and are not Orchestra workers.
- **Memory and schedules.** Facts come from model replies and can echo untrusted content. Routine end dates use UTC. The Orchestra time limit counts active time only.
- **Graph.** Pattern-based. Arbitrary aliases, dynamic paths and Godot UIDs are not resolved.
- **Moli.** Results vary by site; there is no Canvas/WebGL parity.
- **Security.** Autopilot has full OS-user access, the database is not encrypted, and packages are unsigned. See [Security](../SECURITY.md).
