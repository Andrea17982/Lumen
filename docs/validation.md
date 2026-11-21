# Validation and known limits

What has actually been verified for the current source (0.12.0), and which limits remain. Planned work is in the [1.0 roadmap](roadmap-1.0.md). Evidence for earlier versions is in their Git tags and [GitHub Releases](https://github.com/Andrea17982/Lumen/releases).

Fixtures, builds, small real-account checks and owner observations are different kinds of evidence and are reported separately. Lumen is an alpha without an independent security audit.

## 0.12.0

- **Automated:** 293 Rust tests (17 opt-in ignored), 81 frontend tests, 52 production-browser tests, Clippy with warnings denied, formatting and documentation/version checks. CI runs source tests on Windows, macOS and Linux.
- **Interface:** a scripted browser sweep of 228 states (every menu, popover, Settings tab and tour step at 1440, 768 and 390 px, light and dark) checks overflow, clipping, overlapping controls, off-screen popovers, tour targeting and console errors; it found none after the 0.12 fixes. Keyboard focus is visible on every button; the text fields show their caret only.
- **F1–F5:** opt-in Code Mode, per-model opt-in and syntax fallback on Ollama, routed RLM and turn-scoped MCP for Claude Code, Codex and OpenCode. At commit `9317a43`, offline verification passed 293 Rust tests (17 opt-in cases ignored), Clippy, 81 frontend tests and 52 Chromium Playwright tests. Fixtures covered syntax fallback, MCP cancellation and permissions, RLM routing, counters and keyboard access to Code rows. [Configuration and limits](api-access.md)
- **Local Ollama:** Qwen3-Coder gained on repeated file edits and lost on log filtering. The explicit MCP text instructions left the log-filter success rate at 0/5. In the long-context suite the model made zero `llm()` calls. A separate scripted smoke exercised a local subquery. [A/B/C](../reports/bench/2026-11-18/f2-qwen3/summary.md) · [Log-filter rerun](../reports/bench/2026-11-18/f2-mcp-text/summary.md) · [Long context and smoke](../reports/bench/2026-11-18/f3-suite-e/summary.md)
- **Coverage still open:** paid provider/model/effort measurements and a packaged WebView2 pass for Code Mode. The competitor comparison draws on their official documentation. [Comparison](../research_notes/competitors-t3-paseo-paradev-orca.md)

## Carried over from 0.11.0

- **Automated:** 273 Rust tests (14 opt-in ignored), 81 frontend tests, 50 production-browser tests, Clippy with warnings denied, formatting and documentation/version checks. CI runs source tests on Windows, macOS and Linux.
- **Auto:** deterministic rule tests for Parallel/Collaborative and Compete (Default and Budget never compete); an offline corpus of six tasks with tests where Compete solved 5 and Single 3; tests that plain questions are answered in Auto while permissions, secrets and risky or external actions still escalate.
- **Autonomy:** CRLF, Git-filter, concurrency and exact-rollback fixtures; Rewind refusals for shared checkouts and out-of-checkout native turns; memory held for confirmation after untrusted content.
- **Plugins:** manifest validation, refusal of undeclared capabilities, iframe isolation and permission re-approval, in unit and browser tests.
- **Handoff corpus:** constraints and decisions survive two compactions for Claude Code, Codex, Antigravity, OpenCode and the three API adapters.
- **Not done for 0.11:** the native Windows GUI suites and live model checks were not re-run; plugin panels and Git install were not tried in the packaged app.

## Carried over from 0.10.0

These checks were made on 0.10.0 and cover areas 0.11 and 0.12 did not redesign.

- **Native Windows suites** (real WebView2, IPC and SQLite, offline CLI fixtures): service lifecycle and restarts during work, a native question and Orchestra; routing and quota replay; handoff sections and check evidence; execution traces, Stop and the duration line; memory, global-memory confirmation, recurring tasks and recipes through the real GUI and daemon; Orchestra continuation.
- **Local service:** a hard service crash leaves unrelated processes untouched; ordinary model work is not resubmitted after a crash; a process whose identity cannot be confirmed blocks automatic cleanup. [Receipt](data/local-service-0.10.json).
- **Command receipts:** eight concurrent create-session requests with the same command ID created one conversation and returned the same result eight times, also after a service restart; a changed payload was rejected. This covers dispatch, not exactly-once effects of external tools. [Receipt](data/command-receipts-0.10.json).
- **Live recognition** (Claude Sonnet through the official CLI, using an isolated Lumen account slot): 6/6 recurring-task and memory requests, including Italian end dates, weekdays and daily times, a one-off request with no schedule and a project fact; 3/3 project-versus-global memory cases. Reproduce with `node tests/live-routine-recognition.mjs` (opt-in, uses real inference).
- **Earlier live observations:** Codex native questions, file writes and continuation, a two-worker Orchestra, Moli browsing on Codex and Antigravity, local Ollama tool calls, real language-server errors and an opt-in Godot loop. Each was a single configuration.

## Known limits

- **Platforms:** Windows x64 is the only platform with interactive desktop acceptance. macOS and Linux packages pass CI builds, tests and package probes only.
- **Live coverage:** no live quality benchmark; the broader multi-account, CLI-version, quota/restart and repeated-Resume matrix is not certified. A naturally exhausted real-account quota transition has not been observed.
- **Integrations:** universal connector/plugin portability is not delivered. See the [compatibility audit](integrations.md#compatibility-audit-and-explicit-limits). Cursor is explicit Autopilot only; API adapters are buffered and not Orchestra workers.
- **Memory and schedules:** facts come from model replies and can echo untrusted content the model read; review them. A recurring task's end date is end of day in UTC. The Orchestra time limit restarts after a service restart.
- **Performance:** the complete transcript stays mounted; repeated same-process opening drift is unresolved.
- **Graph:** pattern-based, not a compiler. Arbitrary aliases, dynamic paths and Godot UIDs remain open.
- **Moli:** coverage beyond controlled fixtures varies by website, CAPTCHA and rendering; Canvas/WebGL pixel parity is not guaranteed.
- **Security:** Autopilot has full OS-user access; there is no common OS jail and the database is not encrypted. Packages are unsigned. See [Security](../SECURITY.md).

## How to verify

See [Development](development.md) for build, test and native-harness commands. Native harnesses and Playwright share port 1420 and must run serially. Paid or quota-consuming checks are opt-in and never part of CI.
