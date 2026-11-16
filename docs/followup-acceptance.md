# 13 November follow-up acceptance

This record covers the owner's remaining duplication audit, real-account routing, Windows console observation and compiler size/performance comparison. It is separate from the historical 0.8 release-quality experiment and its 13-slot ledger.

## Duplication audit

Reviewed the application/store lifecycle, native provider execution and fallback, account catalogs and usage, shared process creation, settings, the main React controller and its Usage/sidebar controls. Checked callers across `src`, `src-tauri/src`, `scripts` and `tests`, including occurrences of `unused`, `not used`, `duplicate` and `duplication`. An eight-line normalized block scan over 238 source/test/script files found three cross-file candidate groups above 250 characters. This is a candidate detector, not a proof that no semantic duplication remains.

The expected workload is one desktop application with multiple conversations/jobs and native CLI children. Findings below use that workload; multiple simultaneous GUI instances were not load-tested.

### Must fix — resolved

- **Isolated tests could change the installed Windows login item.** Setup/settings called the same OS autostart registration even when using `LUMEN_TEST_DATA_DIR`. An isolated debug fixture now skips that OS mutation. A native test toggles its own setting and checks that the existing Windows Run entry remains identical. Production behavior stays enabled.

### Should fix — resolved

- **Production and Rust fixtures duplicated `Core` initialization.** Adding a semaphore/cache field required keeping two independent constructors synchronized. Both now use `Core::new`, preserving the original limits and initialization.
- **Usage retained a second request-attempt cache and unused error state.** The pending-request map already prevents simultaneous duplicate work. The second map could prevent an Antigravity retry forever after one failure. Removed it and the unused error state; opening Usage can retry, and previous successful readings survive errors.
- **Removed UI left orphan status helpers/styles.** Call-site checks found no production consumers of the old usage-status helpers, session menu button, health pill or usage-state rows. Removed those and the three tests solely asserting the retired helpers. Kept the health dots used by Accounts/Tools. This follow-up reduces the inspected production/test files by 111 lines net, including the autostart fix; no dependency was added.

### Nice to have — intentionally retained

- Native accessibility/execution tests share a short bridge-interception prefix but exercise different faults and ownership boundaries. A generic injection framework would hide the differences without removing product code.
- The composer acceptance script has an older standalone launch/CDP block resembling `nativeHarness`. It also exercises development-module interception and owns its startup evidence. Migrating that test infrastructure is optional; it is not a second production implementation.
- Two reqwest API versions currently serve the local transports and the rmcp 0.13 adapter. This cleanup does not upgrade the local transport dependency merely to unify its package name; transport migration needs its own compatibility evidence.
- Shared select UI and shortened product/design documents were already consolidated in the preceding change. Historical validation reports remain separate records rather than being deleted as duplicate prose.

No additional must-fix duplication finding was established in the reviewed paths. The entire 50k+ line repository, every third-party integration and external CLI implementation were not read line by line.

## Real accounts

The owner authorized bounded live checks. Tests use a disposable workspace and isolated Lumen database; native account-home junctions use existing login state without reading/copying credential files. Junctions are unlinked and noncredential configuration restored afterward.

All three account preflights succeeded. Real Antigravity `gemini-3.8-flash-high` read a probe file (14.117 s), answered a fresh consecutive turn in the same conversation (13.200 s), and was selected by an explicit request inside Auto (19.295 s). A real Auto request selected GPT `gpt-6-luna` and completed (10.712 s). These single observations include model/network latency and are not a provider speed comparison.

GPT currently has usable credits, so a naturally exhausted real-account transition was **not** observed. A separate controlled test injects the native quota error at the Claude/GPT CLI boundary, accepts the two proposed switches and receives the required answer from the real Antigravity account. It completes in 9.082 s. Primary CLI errors/catalogs in that case are fixtures; the final Antigravity inference is real. Offline routing coverage additionally verifies the quality warning, Stop, cleared questions and no automatic replay after an observed tool effect/partial answer.

Evidence: `.codex/verification/live-routing-current.json`, `live-routing-controlled-quota.json`; reproducible opt-in runner: `tests/live-routing-acceptance.mjs` (`LUMEN_LIVE_ROUTING=1`, plus `LUMEN_CONTROLLED_QUOTA=1` for the controlled case). Five tiny real inference requests completed in this follow-up. Earlier failed controlled-fixture preparation is excluded from successful acceptance; the final report records the corrected harness.

## Windows terminal observation

A native Windows test observed the isolated application for 18 minutes 30 seconds with all three real accounts configured and no inference. WinEvent hooks recorded create/show/foreground events by window class and process ownership, without reading user window titles. Six owned GUI events confirm that the hooks were active. No console show/foreground event was attributed to the test application. Windows autostart remained identical after an isolated setting toggle; account configuration was restored.

The global hook additionally observed two short-lived `ConsoleWindowClass` **create** events, without a show/foreground event and without recoverable ancestry, and activation of an existing Windows Terminal process started at 10:03 (before this test). Those are retained separately; no system-wide absence of console creation is claimed.

The planned schedule has three six-minute phases. Actual state sampling during the latter 12 minutes confirms 58 foreground samples, 117 visible-background samples, 156 minimized samples and 386 hidden/tray samples at roughly one-second intervals. The first nominal foreground phase was partly minimized and was not continuously state-sampled. This is an 18-minute observation across operating states, not six uninterrupted foreground minutes or an unlimited guarantee against flashes.

Evidence: `.codex/verification/terminal-summary.json`, `window-events.json`, `window-states.json` and `idle-current.json`. The opt-in native runner is `tests/native-idle-acceptance.mjs` with `LUMEN_LIVE_IDLE=1`; foreground/minimize actions and independent state sampling are performed through the Windows UI during collection.

## Compiler comparison

Both comparison binaries use the same cleaned production source and frontend, a unique application identifier and an empty account list. Baseline: `opt-level=s`, portable CPU default. Candidate: `opt-level=3` plus `target-cpu=native`. Both retain LTO, one codegen unit and stripped symbols. AVX2 support was confirmed on this PC. Two unmeasured warmups precede 20 fresh-process AB/BA pairs. All 40 recorded windows were visible/focused with equal geometry; all 5,000 messages remained mounted and typing/scroll checks passed. Compilation and packaging had finished before timing began.

| Measurement | Size build: median / p95 | Native speed build: median / p95 |
| --- | ---: | ---: |
| Startup, warm OS/profile | 496.1 / 648.0 ms | 504.0 / 624.8 ms |
| Open 5,000 mixed messages | 1,308.8 / 1,379.4 ms | 1,303.0 / 1,321.9 ms |
| Type 12 characters, wall time | 103.2 / 153.2 ms | 103.3 / 136.5 ms |
| Streaming renderer task duration | 564.3 / 637.6 ms | 560.7 / 589.3 ms |
| Tokenizer IPC, 20 warm calls | 26.8 / 28.7 ms | 22.7 / 27.3 ms |
| Executable | 26,509,312 bytes | 36,527,616 bytes |
| Portable ZIP, same resources/compression | 56,178,793 bytes | 59,779,680 bytes |

The combined speed/CPU flags reduce the observed tokenizer median by **15.5%**. UI medians are effectively unchanged; no general UI-speedup or model-inference improvement is established. The speed executable is 37.8% larger and its ZIP is **6.4% larger (+3.60 MB)**. This is a measured tradeoff, not simultaneous proof of maximum speed and minimum size. The requested local CPU configuration is retained for the updated package. Other PCs may lack its instruction set; portable CI builds remain separate.

A single process-tree private-memory snapshot per arm on the large history is 673,558,528 / 679,940,096 bytes. This is not a peak-memory estimate or an idle-memory comparison. Tokenizer samples include IPC and are collected in separate warm blocks per arm; the UI uses alternating paired launches. No cold-OS, other-hardware or sustained-load conclusion is inferred.

Runner: `tests/optimization-performance.mjs`. Detailed launch records, hashes and samples: `.impeccable/followup-acceptance/pair-*.json`, `paired-results.json`, `token-results.json`, `summary.json`. Both frozen binaries were built before three test-only single-element-loop lint fixes; no production-path difference exists between arms. [Sanitized acceptance data](data/followup-acceptance-results.json) includes the source snapshot hash, compiler settings and package checksums.

## Updated packages

The final installer and portable ZIP were rebuilt with the audit fixes and transparent logo. The checksum manifest matches all three distributable files, and the ZIP's executable matches the loose portable executable; bundled Moli and legal notices are present. The benchmark identifier is absent from the final executable.

The previous `builds/latest/Lumen_0.8.0_x64-portable.exe` was still in use, so it was preserved. The new executable and ZIP use the `portable-updated-20261113T150904Z` suffix. The owner must close the previous instance and start the updated copy to use the fixes. The installer is `Lumen_0.8.0_x64-setup.exe`. No remote publication, tag or push was performed.

## Regression checks

71 frontend unit tests, 209 ordinary Rust tests (14 opt-in probes ignored), production frontend build and 44 headless browser regressions pass after cleanup. The native isolated autostart toggle also passes. Clippy with warnings denied, Rust formatting, Prettier, documentation references and native routing/quota-replay regressions also pass.
