# Validation and known limits

This page records what has actually been verified for the current source and which limits remain. Detailed historical checkpoints, protocols and aggregated measurements for 0.7–0.8 are preserved in the [`v0.8.0` tag](https://github.com/Andrea17982/Lumen/tree/v0.8.0/docs) and in commit `05f4066`; they are not repeated here. Planned work is in the [1.0 roadmap](roadmap-1.0.md).

Fixtures, builds, small real-account smoke checks and owner observations are distinct kinds of evidence and are reported separately. Lumen remains an alpha without an independent security audit.

## 17 November - T3 Code command-receipt adaptation

Selected task and approval commands now use persistent, payload-bound UUID receipts. Lost replies and concurrent duplicates return the saved result without repeating dispatch. Pending claims after cancellation/crash are explicitly uncertain and never automatically replayed. This Rust/SQLite adaptation of T3 Code's design preserves [upstream credits, exact sources, changes and MIT text](../src-tauri/resources/legal/t3code/ADAPTATIONS.md).

The full workspace passed 231 Rust tests (14 opt-in ignored) and Clippy with warnings denied. New regressions cover transport-only retry with the same UUID, no retry of domain errors/read commands, concurrent dispatch, argument mismatch, saved errors, invalid/missing IDs, database reopen and preservation of schema-4 messages during migration to schema 5. The native daemon smoke also loses a reply after acceptance, repeats the same command concurrently, restarts the service and checks that only one conversation was created. Documentation checks verify upstream notice hashes; release helper checks include adaptation notices. This proves the covered dispatch behavior, not exactly-once external effects or every adapter/account combination.

The earlier 16 November package receipt below describes the initial schema-4 build, not this follow-up source. Current service migration/recovery requirements are in [local-service.md](local-service.md).

## 16 November - 0.10 core and local service

- **Implemented:** headless core, authenticated local service, one owner per data directory, transactional event/checkpoint recovery, bounded live-worker compaction, durable quota intentions and explicit process-safe recovery. The GUI closes independently; Stop Lumen and quit is available in the tray and General settings.
- **Checks:** 227 Rust tests, 14 opt-in ignored; 76 frontend and 44 browser tests; Clippy with warnings denied, formatting and docs/version checks. All twelve adapter suites passed. Five native lifecycle scenarios cover full GUI restarts during normal work, a native question and Orchestra; durable quota cancellation; and a hard service crash beside an unrelated process. Four focused Orchestra continuation cases and five actual installed OpenCode 2.2.7 ACP probes passed without paid inference. General-service shutdown stops both processes without a tray and fits light/dark layouts at 480/1240 px (480 is emulation; native minimum window width is 720).
- **Recovery safety:** ordinary model work is not silently resubmitted after a service crash. Kernel ownership contains service processes. A legacy live/unknown PID blocks automatic cleanup; no approximate PID/start-time match authorizes termination. A Windows regression leaves such a process untouched before explicit fixture cleanup.
- **Cross-platform:** source CI passed on Windows, macOS and Linux. Branch-only release packaging passed installed Windows helpers, universal mounted DMG helpers and extracted DEB/AppImage helpers. This does not claim interactive macOS/Linux desktop acceptance. The public latest release remains 0.9.0.
- **Local artifacts:** the 0.10.0 installer, portable ZIP, loose GUI and daemon in builds/latest passed four SHA-256 entries, ZIP CRC/safe extraction, built/loose/ZIP/prepared-sidecar identity, NSIS source identity, Windows version fields, all browser/legal resources, and an unchanged 428-file compiled-source freeze. Release helper modes and a real headless release-service startup/drain/restart used private temporary data, never the real user database. A warm local 100-call RPC/ten-second idle sample is recorded without claiming a GUI or provider performance comparison.

[Detailed receipt](data/local-service-0.10.json) - [Service operations, migration and update procedure](local-service.md). These are local CPU-native Windows builds. No new paid inference, naturally exhausted real-account reset or independent security audit is claimed.

## 16 November — 0.9 functional acceptance

- **Implemented:** three official API adapters with OS keychain, shared tools, four access profiles, durable estimated-cost budgets and explicit Auto opt-in; OpenCode official ACP/session-policy bridge; nine-adapter capability/event/error/permission/cost matrix. [Release scope](release-notes/0.9.0.md) and [API boundaries](api-access.md).
- **Checks:** 225 Rust tests passed, 14 opt-in ignored; 73 frontend tests; 44 production-browser tests; all-target Clippy with warnings denied; formatting and docs/version consistency. The twelve-suite serial [adapter corpus](development.md#09-adapter-corpus) passes the four fundamental agents and three API wire adapters, plus Cursor/shared integration/continuity/layout regressions. Nine provider choices stay on one row at five widths in both themes, with local scrolling where necessary.
- **Installed runtime:** OpenCode 2.2.7 with real ACP/session API and a temporary, deterministic loopback provider. Native policies deny forced writes/shell/outside-project access in restricted modes; Allow once/Deny reaches the actual tool. A project MCP oracle and native continuation preserve results without a repeated effect. Other providers are denied by native test-project policy, so no paid model call is made.
- **API evidence:** actual Windows WebView2/IPC/keychain/SQLite with fake UUID-owned keys and local HTTP fixtures. All three formats, permissions, Moli/graph/MCP result oracles, Google thought signatures/aliases, Auto revocation, cumulative budgets, known rejection refunds and uncertain cancellation are covered. Host command failures remain tool errors, and Stop kills owned descendants. Settings are rendered at 480/1276 px in both themes, with cleared password input and renewed confirmation after a model/rate change.
- **Scope:** no paid API/account entitlement or model-quality claim, no naturally exhausted real-account reset, no new clean-machine installation or macOS/Linux desktop validation. Cursor installed metadata still returned no model catalog in the earlier checkpoint. [Explicit integration limits](integrations.md#compatibility-audit-and-explicit-limits).

Summary: [compatibility-0.9.json](data/compatibility-0.9.json). Detailed local receipts: `.codex/verification/09-*`. Functional acceptance used isolated native debug processes and the production frontend, not the real user database. At the 0.9 checkpoint, its installer, loose portable and ZIP (now archived when replaced in `builds/latest`) passed exact-artifact verification: three SHA-256 entries, ZIP CRC, built/loose/ZIP executable identity, installer source identity, all 167 browser/legal resources, and unchanged hashes for 411 frozen source files. Product/FileVersion are 0.9.0. Release graph/browser MCP helpers, bundled Moli and upstream legal-file hashes pass without opening the real application database. The artifacts are local CPU-native Windows builds, not a remote publication or a clean-install test.

## Prior 0.8.1 checkpoints

[Historical scope and evidence](release-notes/0.8.1.md) records model-class continuity, opt-in quota reset, initial print-only OpenCode and Cursor adapters, typed failures and their Windows packaging. Those limitations describe their checkpoint; the current matrix above supersedes them. Earlier artifacts remain in `builds/archive/2026-11-16` when replaced. The canonical 1.0 roadmap retains all owner decisions from the completed interview and shared conversation.

## Earlier checkpoint — 13 November 2026 follow-up

This earlier follow-up retires Gemini CLI, consolidates single-choice controls into a shared selector, replaces the logo with the approved satin-glass mark, simplifies usage/settings/conversation controls, adds project rename/move/delete, and fixes quota fallback and the Antigravity stream lifetime.

- **Local checks:** 209 Rust tests passed (14 opt-in ignored), 74 frontend unit tests and 44 browser tests against the production bundle. Native Windows/WebView2 suites `tests/native-routing.mjs`, `tests/native-quota-replay.mjs` and `tests/native-menu-layout.mjs` (10 layout scenarios across widths and themes) pass with isolated offline fixtures. Cold Vite dev-server runs that timed out are not counted as passing.
- **Routing and quota:** explicit Antigravity routing without classification, Google-only models, three-account credit exhaustion with per-attempt consent, the weaker-model warning, Stop without another send, partial-output retention and no replay after observed file work.
- **Real accounts (owner-authorized, five tiny inference requests):** Antigravity `gemini-3.8-flash-high` read a probe file, continued the same conversation and was selected by an explicit request in Auto; Auto selected and completed with GPT `gpt-6-luna`. A naturally exhausted real-account quota transition was **not** observed; a controlled test injected the quota error and completed on the real Antigravity account. Single observations include network/model latency and are not speed comparisons.
- **Windows console observation:** 18 min 30 s across foreground, background, minimized and tray states with all three accounts configured and no inference. No console show/foreground event was attributed to Lumen; two unattributed short-lived console creations elsewhere on the system are recorded, so no system-wide absence is claimed.
- **Compiler comparison:** `opt-level=3` with `target-cpu=native` against `opt-level=s` over 20 fresh-process AB/BA pairs showed differences within noise for startup, opening 5,000 messages and typing. Local release builds use level 3 plus the build CPU's instruction set; CI distribution builds keep the portable baseline target.
- **Not verified:** the owner's reported focus-stealing terminal and live Antigravity failure on the running installation; other platforms; all Windows scaling factors.

Full record: [13 November follow-up](followup-acceptance.md).

## Published 0.8.0

Tag `v0.8.0` (commit `47ed088`), published 13 November 2026 with explicit integration limits approved by the owner. [Release](https://github.com/Andrea17982/Lumen/releases/tag/v0.8.0) · [Changelog](release-notes/0.8.0.md).

- 203 Rust tests on Windows, 195 on macOS and 195 on Linux (14 opt-in ignored per platform), 72 frontend unit tests, 40 browser tests and 56 native Windows scenarios across eight suites, plus formatting, Clippy and documentation/version checks.
- Windows installer and portable ZIP, universal macOS DMG (`arm64` and `x86_64`) and Linux DEB/AppImage pass helper probes after installation/extraction: graph/browser MCP discovery, bundled Moli and license-file hashes. All five downloaded packages match the public SHA-256 manifest. Packages are unsigned.
- Fresh-process Windows release gates: opening and typing medians within the fixed 10% regression gate against baseline `01bf144`; eight historical-content access gates pass. Earlier scoped gains (63.1% less typing work, 25.6% faster opening, 37.2%/57.4% shorter Claude/Codex metadata calls) belong to their original workloads and are not new measurements.
- Context and graph: the fixed TypeScript/JavaScript/Godot import corpus passes 23/23 expectations; exact duplicate instructions and unchanged evidence are removed from measured prompts.
- Accessibility: targeted Windows keyboard/focus/modal corrections; Narrator labels confirmed understandable by the owner. Not a WCAG or general screen-reader certification.

## Earlier live observations (0.7)

Codex native questions, file writes, same-thread continuation, graph calls, a two-worker Orchestra and Moli browsing were observed on real accounts; Antigravity completed a Moli navigation/capture turn; installed local Ollama `llama3.2` called Moli and returned page data; real Rust/TypeScript/C#/Godot language-server errors and an opt-in Godot Discover/Play/Stop/Reload/capture loop were exercised in isolated projects. Each was a single configuration, not every version or identity.

## Known limits

- **Integrations:** universal connector/plugin portability is not delivered. See the [compatibility audit](integrations.md#compatibility-audit-and-explicit-limits).
- **Platforms:** Windows x64 is the only platform with native desktop acceptance. Published 0.8 macOS/Linux packages had CI builds/tests/package probes; no new 0.9 CI or native desktop acceptance was run on those platforms.
- **Live coverage:** no live quality benchmark; Claude live inference and the broader multi-account, CLI-version, quota/restart and repeated-Resume matrix are not certified.
- **Performance:** repeated same-process opening drift is unresolved; the complete transcript stays mounted because the virtualized prototype failed search/selection/history gates. Rust Markdown/ANSI, warm CLI pools and positional metadata were not integrated.
- **Graph:** approximate, not a compiler. Arbitrary aliases, complex comments/strings, dynamic paths and Godot UIDs remain open.
- **Moli:** website, CAPTCHA and rendering coverage beyond controlled fixtures; Canvas/WebGL pixel parity is not guaranteed.
- **Security:** Autopilot has full OS-user access; there is no common OS jail and Lumen does not encrypt its database. See [Security](../SECURITY.md).

## How to verify

See [Development](development.md) for the build, test and native-harness commands. Native harnesses and Playwright share port 1420 and must run serially. Paid or quota-consuming checks are opt-in only and never part of CI.

## 0.9 package weight

With the same local release optimization/CPU flags as 0.8.1: portable executable **36.33 MiB** (+3.24%), installer **39.75 MiB** (+0.68%) and portable ZIP **57.94 MiB** (+1.24%). These are artifact sizes, not a new startup/runtime performance benchmark. Exact bytes and hashes are in the [acceptance summary](data/compatibility-0.9.json).

## Published 0.9.0

[v0.9.0](https://github.com/Andrea17982/Lumen/releases/tag/v0.9.0), source `2dd7190`, published 16 November 2026. CI passed on Windows/macOS/Linux, including installed Windows, universal mounted DMG and extracted Linux package graph/browser/Moli/legal helpers. All five downloaded distribution packages and their SHA-256 manifest match GitHub asset digests; Windows ZIP CRC, safe extraction, version and release helpers were also verified locally. These portable CPU distribution builds are separate from `builds/latest` CPU-native artifacts. No additional live model inference or interactive macOS/Linux desktop acceptance is claimed. [Publication receipt](data/release-0.9.json).
