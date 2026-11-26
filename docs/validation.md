# Validation and known limits

Automated checks, live observations and open limits. Evidence for earlier versions is in their Git tags.

## Automated

- Rust tests, linting, formatting, the daemon smoke test over IPC, and documentation and version checks run in CI. See the [development guide](development.md#checks).
- **Permissions.** A matrix checks every access profile against read, write, command, destructive command and `git push`, for both classic and Code Mode calls.
- **Auto.** Deterministic rule tests for Parallel, Collaborative and Compete. The Windows daemon smoke test also drives the quota ranking across two accounts of the same provider: more remaining quota, an earlier weekly reset or more weekly headroom each win in every priority, equal or missing data keeps the current account, and only Budget trades providers at an equal model tier. It also verifies the downgrade consent end to end: with only a weaker model left and the account that could serve the task out of quota, the question names the blocked provider and when its quota resets, and the turn continues only on an explicit answer. Unit tests cover the OpenCode emergency chain: the catalogue maps real names and reasoning variants, subscription models come first and console-credit ones never enter Auto, Go metres map to contract windows, a pinned model is the only one used, and a missing tier goes up before down. Cross-provider quota comparisons pair windows by duration.
- **Auto recovery.** A Windows daemon smoke test with offline Codex verifies model and effort on the native wire, quota cooldown across a crash, and one native continuation in the original project's worktree. A later restart preserves the completed turn and its effects.
- **Autonomy.** Fixtures for CRLF and Git filters, concurrent checkpoints, exact and undoable restore, Rewind refusals, and memory held after untrusted content.
- **Handoff.** Constraints and decisions survive two compactions on all core agents and APIs.
- **Plugins.** Manifest validation, refusal of undeclared capabilities, re-approval when permissions grow, and panel fetches refused for IP addresses and `localhost` names.

## Desktop client

- **Unit tests** (Vitest) for the sync cache (snapshots, event order, deduplication, reconnection), the transcript model, attention and recovery rules, sidebar ordering, Orchestra plan edits, plugin theme filtering and both language catalogs.
- **Browser tests** (Playwright) drive the client against the real debug service with offline CLI fixtures: streaming and Stop, approvals and native questions (a secret answer is kept nowhere), quota resume, rewind, projects and conversations, accounts (an API key never reaches the page or any file of the service), Orchestra from questions to the team result and resume after a crash, settings, MCP, skills, a plugin panel confined to its sandbox, `/every`, task branches with Apply and Discard. Axe accessibility checks run on each surface; the gallery has keyboard, 150 % text and screenshot checks.
- **Long transcripts.** 5,000 messages: first messages in about 0.3 s, all mounted in 4 to 5 s, 95th-percentile frame and typing gaps under 25 ms on the owner's machine. Windows UI Automation reaches the first and last fixture messages. Text inside an off-screen message can be absent from the accessibility tree.
- **Real app** over WebDriver. The shell starts the bundled service, relays events and restarts it after a crash. Reopening the shell restores the last conversation in its project and shows the partial reply once. A sandboxed plugin panel cannot call the shell.
- **Security review** of the panels, the plugin protocol and the shell by a review agent; findings fixed before release. Not an independent audit.

## Worktree and Publish

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

## Codex correction benchmark

Two paired synthetic tasks on Codex GPT‑6.2 Sol at low effort used 79.7% fewer input tokens outside cache after verification output was compacted. Both arms passed all 11 acceptance checks. [Method, token counts and limits](release-notes/0.13.0.md#codex-benchmark).

## Earlier live observations

These are single configurations, not certified:

- recurring-task and memory recognition with Claude Sonnet (6/6, 3/3);
- Codex native questions and continuation;
- a two-worker Orchestra run;
- Moli on Codex and Antigravity;
- Ollama tool calls;
- real language-server errors.

## Known limits

- **Platforms.** Interactive desktop acceptance has been recorded on Windows x64. See [releases](release-notes/README.md) for packaging.
- **Screen readers.** The transcript is checked through UI Automation; a full NVDA and Narrator pass by a person is still to do.
- **Live coverage.** The paired correction probe covers two tasks. The multi-account, CLI-version, quota and restart matrix remains uncertified.
- **Integrations.** Portability is limited to the documented subset ([Providers](providers.md#shared-integrations)). Cursor runs in Autopilot only. Droid has no Ask first. API adapters buffer responses and are not Orchestra workers.
- **Memory and schedules.** Facts come from model replies and can echo untrusted content. Routine end dates use UTC. The Orchestra time limit counts active time only.
- **Graph.** Pattern-based. Arbitrary aliases, dynamic paths and Godot UIDs are not resolved.
- **Moli.** Results vary by site; there is no Canvas/WebGL parity.
- **Security.** Autopilot has full OS-user access, the database is not encrypted, and packages are unsigned. See [Security](../SECURITY.md).
