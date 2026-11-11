# Turn waiting and CLI/provider startup — 0.8

Protocol fixed before measurement on 11 November 2026. Preserve the current uncommitted work, 0.7.0 version and prior evidence. **Zero new inference calls**: the existing live ledger remains at 13 spent slots. No authentication retry, new paid pair, model download, warm process pool or release publication is authorized by this experiment.

## Attribution protocol

- Instrument content-free monotonic offsets, separately from wall-clock execution timestamps: command preparation before acceptance, stage catalog/capacity/dispatch, process spawn, protocol readiness, request enqueue/acknowledgment where observable, first normalized event/text and return. Enqueue is not proof of provider receipt; stdout protocol readiness is not a model response or network first byte.
- Repeated Windows native offline turns: 20 samples per Claude/Codex initial and same-account continuation path. Real IPC/storage/processes and controlled CLI fixtures, isolated data, no original accounts. Record acceptance round trip, completion, stage milestones and persistence; retain first-process/cache distinctions. These are harness costs, not live provider speed.
- Actual installed CLI measurements: 20 version and metadata initialization/catalog samples per available Claude/Codex provider, alternating order, fresh processes and isolated configuration without copying credentials. Metadata protocols never submit a user message or turn/start. Stop the provider series on prerequisite failure; retain it and do not treat missing authentication/catalog as successful latency.
- Report medians/p95 with 20 samples, first process separately, no discarded slow samples. Preserve raw traces under ignored `.impeccable/turn-latency/`; publish only sanitized aggregates, statuses and binary/version fingerprints.
- Primary outcome is **attribution**, not a speedup claim. Provider inference/TTFT remains unmeasured without remaining live budget. Reusable CLI processes are conditional follow-up: their benefit and isolation/cancellation/quota behavior need a separate prototype and checks, not an assumption based on --version.

## Results — 11 November 2026

Both installed providers complete all 20 metadata samples: **Claude Code 2.1.314**, **Codex CLI 0.167.0-alpha.4.1**. No user message or `turn/start` is submitted by this probe. Configuration is isolated and no credentials are copied. [Sanitized aggregate record](data/turn-latency-0.8-results.json).

| Installed CLI | `--version`, median | Spawn, median | Process → initialize acknowledgment, median / p95 | Metadata call including shutdown, median / p95 | Shutdown, median |
| --- | --- | --- | --- | --- | --- |
| Claude | 88.6 ms | 4.5 ms | 531.3 / 635.4 ms | 953.3 / 1,095.8 ms | 405.0 ms |
| Codex | 111.7 ms | 4.3 ms | 185.5 / 229.9 ms | 588.3 / 719.1 ms | 387.3 ms |

Process creation is a small part of this metadata path. Runtime startup plus protocol initialization takes much longer; these two cannot be separated using the exposed acknowledgment alone. Shutdown is measured independently from the completed query through stdin close/process-tree termination/wait. Source inspection confirms the Windows path uses `taskkill /T /F`; the whole measured shutdown span is not attributed exclusively to that command. These are fresh CLI processes in a warm OS/profile history, not cold-start guarantees or model-execution timings. Medians of individual spans need not sum to the median total.

The **80-turn isolated Windows release** uses offline CLI fixtures, with 20 initial and 20 same-account continuation turns per provider:

| Offline provider path | Preparation median | Acceptance IPC median | Completion observed median | Process → protocol readiness median |
| --- | --- | --- | --- | --- |
| Claude initial | 1 ms | 8.7 ms | 987.4 ms | 298.5 ms |
| Claude continuation | 1 ms | 9.3 ms | 995.3 ms | 300.5 ms |
| Codex initial | 1 ms | 8.7 ms | 906.3 ms | 280.0 ms |
| Codex continuation | 1 ms | 9.0 ms | 905.1 ms | 283.0 ms |

All samples are retained, including the first account uses: acceptance 734.3/686.2 ms and completion 1,807.4/1,938.8 ms (Claude/Codex). The existing five-minute catalog cache explains why later preparation is much smaller; **no cache speedup was implemented in this tranche**. Codex also has a schema cache after its first version-checked use. Initial/resume refer to native session continuity; both paths still start a new CLI process. PowerShell fixture startup and teardown dominate the offline totals, which must never be presented as real Claude/Codex provider response times.

The release executable SHA-256 is `4a9e8481d23eb16cbc0353575b45b8262fb19b829467cde2e5fd290112082244`. It contains production frontend assets under a separate benchmark application identity/profile. Coding-turn workspaces are outside the original repository's Git ancestry. Actual CLI timings use a Rust debug test driver and the installed CLI binaries; they are distinct from Lumen release-turn measurements.

## Instrumentation and coverage

Accepted direct submissions persist `preparationMs` on the existing queued execution-phase record. Stage snapshots retain version 1 and add a bounded `timings` list of named millisecond offsets (maximum 64). A process-wide `Instant` origin stamps transport milestones **at emission**, so a queued callback cannot move startup work into a later phase. Stored values are relative offsets; no prompt, response, path, command, account/native-session identifier or credential is added. Unknown labels and invalid/future timestamps are rejected. Existing snapshot checkpoint/finish publication persists them; the new signals do not add individual UI events or SQLite writes.

Boundaries include catalog, capacity/dispatch, adapter setup, spawn start/process created, protocol readiness, request enqueued, Codex version/schema/thread readiness and turn acknowledgment, first normalized event/text, and return. Request enqueue means the transport accepted the frame, not that the provider received or executed it. Normalized event/text timestamps are controller observations, including existing scheduling/text buffering (up to the 24 ms batch); they are not network first-byte timestamps. A metadata session event can precede the user request. Do not derive provider TTFT from it. Repeated labels must be interpreted per attempt rather than silently merging retries.

Preparation covers successful direct submissions, including local validation/classification/catalog work before acceptance. Auto classifier inference, rejected preparation, queue/context before stage entry, Orchestra worker scheduling and real provider/network/tool time are not independently attributed here. Capacity-phase spans also include local bookkeeping. Completion observed through IPC includes up to the 25 ms polling interval plus IPC; stage return is a separate measurement. Native session, permission, retry/cancel and account-routing policies remain unchanged.

## Decision and next candidate

This tranche completes the requested attribution baseline and instruments future authorized turns. It does **not** establish real model TTFT or official-app parity. Those need available live budget; the original 13-slot ledger is unchanged.

The metadata shutdown span is now a concrete Windows optimization candidate, ahead of a general persistent-process pool: compare bounded graceful metadata shutdown against the current safe process-tree termination path, preserve cancellation/descendant cleanup and require the existing performance/correctness gates. This is a follow-up candidate, not an implemented optimization. Reusing provider processes additionally needs account/folder/model/permissions, concurrent-session, crash/quota and idle-resource isolation checks; native resume alone does not satisfy them.

## Verification and reproduction

For an independent repetition, choose a new ignored root and set both `LUMEN_TURN_LATENCY_ROOT` and `LUMEN_PERF_ROOT` to it, using its config, executable and metadata output path in the commands below. Preserve the completed root; the no-accounts guard deliberately rejects reusing its fixture account database as a fresh run.

The instrumentation passes 152 Rust tests (14 opt-in probes ignored in the normal run), Clippy with warnings denied and Rust formatting. Four debug smoke turns validate timeline persistence and native continuation. Eight native execution scenarios and seven native quality/account/audit scenarios pass, including actual silence/cancel, provider error, questions and saved internal Orchestra interview observations. The 80 release observations separately verify completion, preparation and source-stamped startup/request milestones; raw latency control events do not enter stored message activity.

1. `LUMEN_PERF_ROOT=.impeccable/turn-latency node tests/performance-prepare.mjs` creates a fresh benchmark identifier/config. In PowerShell, set environment variables with `$env:NAME='value'`.
2. With the transcript experiment flag unset, build `npx tauri build --no-bundle --config .impeccable/turn-latency/config.json` and freeze the executable under this root's `candidate/lumen.exe`. This is a benchmark artifact, not a distributable normal-identity build.
3. Set `LUMEN_CLI_LATENCY_OUTPUT` to the absolute root `cli-raw.json` path; run `cargo test --manifest-path src-tauri/Cargo.toml --lib installed_cli_metadata_latency -- --ignored --nocapture`. Only this metadata test is selected; never run all ignored tests. It stops a provider's series on prerequisite failure and never authenticates or submits inference. Keep configuration/raw records local.
4. Set `LUMEN_LATENCY_EXE` to the frozen release executable and run `node tests/native-turn-latency.mjs`. It prepends offline fixtures only in its own process, asserts isolated data/no initial accounts, and uses a separate temporary workspace. Unset the executable to use debug mode; `LUMEN_LATENCY_SMOKE=1` writes a separate four-turn smoke report.
5. Run `node tests/turn-latency-report.mjs`. It requires complete release/metadata series and emits only aggregate data to `docs/data/turn-latency-0.8-results.json`. Run measurements serially without unrelated builds/tests; retain first/failed samples. Raw data, isolated DB/profile/config, captures and binaries remain under ignored `.impeccable/turn-latency/`.

No version bump, publication, live model call or new process pool occurred. Cold OS startup, actual authenticated provider latency, Auto/Orchestra timing attribution and non-Windows desktop results remain open.
