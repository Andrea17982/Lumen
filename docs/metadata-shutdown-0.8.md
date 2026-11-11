# Windows metadata CLI shutdown

Protocol fixed on 11 November 2026 before the candidate. This checkpoint is included in the owner-authorized **0.7.1 — Performance Update**; the remaining 0.8 roadmap stays open. No new model inference; the existing 13-slot live ledger is preserved.

## Fixed comparison

Freeze the current metadata-only Rust probe executable before changing shutdown. Run **A/B/A/B**, 20 samples per provider per block, alternating Claude/Codex order within each block: 40 metadata samples per provider/arm. Same installed CLI versions, isolated fresh configuration, warm OS distinguished from process creation. Keep every sample and first-use observations.

Primary gate: at least **20% lower median complete metadata-call time for both CLIs**, including any added process-supervision startup cost. Secondary: query, spawn, initialization and cleanup spans; investigate reproducible >5% regressions outside cleanup. No model latency or cold OS guarantee follows. Baseline and candidate are debug Rust drivers of the actual installed CLI, not app/CLI inference benchmarks.

Candidate: Windows-only Job Object supervision attached while the root is suspended, before CLI code can spawn descendants; keep hidden-window flags. Close metadata stdin and allow up to **80 ms** for normal root exit, unless cancelled/failed. Then terminate any remaining job members and reap the whole group. A dropped future closes the kill-on-close job. Fail closed if supervision cannot be established. Other platforms and model-turn shutdown retain their existing paths.

Hard gates: normal EOF, EOF-ignoring root, parent exit with surviving child, cancellation during query/cleanup, task abort/drop, separate concurrent groups and no collateral termination. Record live process disappearance using handles/PIDs from owned fixtures. Successful metadata must remain correct; cancellation must not become success. No process retry, account reuse or auth change.

Raw executable/CLI records stay ignored under `.impeccable/metadata-shutdown/`. Publish sanitized aggregates only. Release checks, native regressions and cross-platform CI/packaging follow accepted results before publication.

## Result and integration

All **160 metadata calls and 160 version probes** in the fixed A/B/A/B series pass, with stable CLI versions/model counts. The complete-call gate passes for both installed CLIs:

| CLI | Baseline complete-call median | Candidate median | Reduction | Cleanup baseline → candidate |
| --- | --- | --- | --- | --- |
| Claude Code 2.1.314 | 1,082.1 ms | 679.0 ms | **37.2%** | 427.6 → 44.2 ms |
| Codex CLI 0.167.0-alpha.4.1 | 673.2 ms | 286.6 ms | **57.4%** | 422.7 → 17.5 ms |

Suspended assignment, job creation and thread resumption add measurable setup cost: spawn/supervision median 8.7 → 50.4 ms for Claude and 8.3 → 51.5 ms for Codex. This roughly 42 ms tradeoff is required to prevent descendants starting before supervision and is already included in the net complete-call improvement; it is not hidden as an unrelated regression. In the candidate, the root exits within the grace window in 36/40 Claude and 40/40 Codex observations; remaining Claude cases use forced job cleanup. First-use samples, p95, individual blocks and driver hashes are retained in the [aggregate record](data/metadata-shutdown-0.8-results.json). A separate 20-per-provider confirmation verifies the final 0.7.1 source/driver after extracting the testable query helper: 754.8/335.6 ms complete calls and 41.9/16.8 ms cleanup, still 30.2%/50.2% below the preserved baseline medians. These varying absolute values remain visible; no universal constant speedup is promised.

Implementation uses [process-wrap 10.0.1](https://docs.rs/process-wrap/10.0.1/process_wrap/) to launch suspended, attach a kill-on-close Windows Job Object, then resume while retaining `CREATE_NO_WINDOW`. Its job controls the descendants before any CLI code runs. [Windows Job Object inheritance and lifetime rules](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) describe the normal Win32 descendant model; this is process-lifetime management, not a new OS permission sandbox. Unsupported supervision fails before executing the suspended CLI; no unprotected retry is made.

After EOF, only the root is given the bounded grace window. Remaining job members are then stopped and the first full job wait reaps them, including children whose parent already exited. Cancelled/failed queries skip grace; cancellation during grace is still returned as cancellation. Drop/abort closes the job. Root observation uses a narrowly scoped private raw-child borrow solely for wait/try-wait, without replacing handles or bypassing job termination. Non-Windows metadata and every model-turn shutdown keep their prior behavior.

Six Windows process tests cover ordinary EOF, an EOF-ignoring tree, surviving descendants after parent exit, cancellation during grace, aborted lifetime with an independent job left intact, and the actual catalog query on both cancellation and task abort. The full Windows Rust suite passes 158 tests (14 opt-in probes ignored), with Clippy denying warnings. The 0.7.1 frontend passes 72 unit tests and 29 browser regressions. Native composer/execution/quality regressions and release packaging are recorded in [validation](validation.md); cross-platform build evidence remains distinct from desktop acceptance.

No new inference or process pool is introduced. Complete metadata-call speedup is not model TTFT or a promise that every turn improves by the same amount. Earlier 0.8 experiment records retain their original checkpoint versions and hashes; accepted changes are packaged as the owner-authorized 0.7.1 Performance Update while the 0.8 roadmap remains open.

## Reproduction

Freeze the pre-change `latency_probe::installed_cli_metadata_latency` test executable as `baseline-test.exe`, the candidate as `candidate-test.exe`, and use separate explicit `LUMEN_CLI_LATENCY_OUTPUT` paths for baseline-1/candidate-1/baseline-2/candidate-2. Run only this ignored test by exact name; it initializes and reads metadata but never sends a user prompt or `turn/start`. The final version uses `final-test.exe` and `final-confirmation.json`. `node tests/metadata-shutdown-report.mjs` validates the complete records and writes the aggregate JSON. Compile/test work must finish before measurements begin. Keep actual CLI calls separate from offline fixture tests, and preserve all failed or first samples.
