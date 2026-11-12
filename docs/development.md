# Development, tests and releases — 0.7

## Build

Use Node.js 22.12+ / supported Node 24, current stable Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). Windows needs the MSVC toolchain, Windows SDK and WebView2. Linux packaging uses Ubuntu 24.04 for the capture/PipeWire dependencies. Lockfiles are committed; a minimum Rust field is not a promise about future transitive dependencies.

```sh
npm ci
npm run tauri dev
```

`npm run dev` is a UI-only preview without login/inference. Both dev/build prepare Moli from the pinned platform manifest, verify archive hashes, retain license notices and reuse verified resources. A clean build needs network access to acquire those artifacts. Direct Rust tests require `node scripts/prepare-moli.mjs` first if resources are absent.

| Target | Command / output |
| --- | --- |
| Windows | `npm run build:release` → installer, loose executable/resources, portable ZIP and checksums in `builds/latest/` |
| macOS | `npm run tauri build -- --target universal-apple-darwin --bundles dmg` after installing both Rust targets |
| Linux | `npm run tauri build -- --bundles deb,appimage` with system prerequisites |

The Windows portable distributable is the ZIP, not an executable copied without its `resources/moli` directory. Signing and a separate updater are not configured.

## Checks

`node tests/native-shared-integrations.mjs` adds isolated Windows acceptance for discovered installations, stdio relay/process-tree cleanup, native account RPC without model turns, original skill resources, restricted catalogs, unsupported APIs and a Claude → ChatGPT-account tool call with consent shown by Lumen itself. It uses offline fixtures, never real credentials or inference. Run it serially with all other port-1420 suites. [Scope and remaining requirements](shared-capabilities-0.8.md).

After a debug Cargo build, `node tests/native-connectors.mjs` checks MCP form/URL consent, typed validation/retry, decline, resolution/Stop expiry and Claude Ask approvals in real Windows WebView2/IPC with offline CLI fixtures. It never signs in or opens the fixture OAuth URL. Run it separately from Playwright and other native harnesses: they share port 1420, and an owning harness closing its server interrupts the others. See [scope and results](connector-interactions-0.8.md).

```sh
npm run check
npm run format:check
npx playwright install chromium
npx playwright test --workers 2
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
node tests/docs-check.mjs
```

The docs check validates local Markdown links/images, the current release notes and version consistency across application manifests. Historical notes remain explicitly historical. Rust tests use real subprocesses, disposable Git projects, local protocol peers and the bundled Moli fixture. Opt-in live tests are excluded from ordinary CI; [Validation](validation.md) distinguishes these scopes.

## Native Windows and screenshots

Keep Vite running, build `cargo build --manifest-path src-tauri/Cargo.toml`, then run `npm run test:native`. The harness uses fresh temporary app data, controlled CLI fixtures, a local Ollama API simulator and loopback WebView2 debugging. It does not authenticate or call remote inference. `LUMEN_TEST_DATA_DIR` is debug-only; release uses normal app data.

`node tests/capture-release.mjs` captures the 0.7 gallery from the native Windows application with isolated demonstration data. It never opens real account directories. Screenshots are actual UI captures; captions must disclose fixture data and distinguish them from live acceptance evidence. Close the owned app/server when finished. Do not expose test/debugging ports to the network.

`node tests/native-composer.mjs` is a focused Windows/debug acceptance harness after the same Cargo build. It starts a dev server only when one is absent, uses fresh SQLite/account/WebView2 data and offline Claude fixtures, and stops its owned processes. It checks per-chat text/attachments, immediate pending delivery, duplicate-send protection, delayed acceptance across chat switches, rejected-send recovery, uncertain readback, new-chat creation and delayed Compact. A page-scoped dev-module transport seam introduces controlled delays/failures without modifying Tauri internals or production source; accepted turns still use real IPC/storage/CLI fixtures. Drafts currently survive chat switches in the same window, not reload/restart. Results and light/dark pending-state screenshots are written under `.impeccable/review/`; this is not a release performance baseline or live model comparison.

`node tests/native-execution.mjs` uses the shared `tests/native-harness.mjs` fixture for focused execution-observability acceptance: actual question/answer, provider return without tools, explicit tool lifecycle, provider failure, a real 30-second silence, read-only refresh/cancel, delayed refresh after navigation, internal Orchestra trace persistence and Italian/dark rendering after reload. Run native harnesses serially so a dev server owned by one harness is not closed while another uses it. They use isolated data and offline CLIs, without live inference. Test shutdown tolerates only the expected owned-webview-close acknowledgment race. Results/captures are under `.impeccable/review/native-execution-results.json` and `execution-*.png`.

## Windows release performance experiments

`node tests/performance-prepare.mjs` prepares a separate benchmark identifier/profile and source excerpts before candidate edits. Build `npx tauri build --no-bundle --config .impeccable/performance/config.json` and freeze the reference executable under `.impeccable/performance/baseline/lumen.exe`; freeze later candidates under `candidate/lumen.exe`. These binaries have a benchmark identity and are not normal distributable releases. New benchmark configurations also use a distinct product name for OS integration names.

`node tests/performance-native.mjs baseline` / `candidate` / `confirmation` collect 20 native repetitions per 100/1,000/5,000-message short/mixed workload, with embedded production assets and no accounts/inference. `confirmation` runs the frozen baseline again. `performance-scroll.mjs` investigates scroll independently; `performance-acceptance.mjs` verifies content/actions on the exact candidate hash and cleans its own generated fixtures. Run native harnesses serially, separate unrelated CPU-intensive work from timing runs, and retain failed/inconclusive observations. Node with `node:sqlite` is needed for native fixture DB writes; Windows Store Python's redirected AppData is not used.

`performance-report.mjs` emits sanitized aggregate JSON under `docs/data/`. Full-workload raw samples and focused-window summaries, fixtures, WebView data, executable/source snapshots and captures stay ignored under `.impeccable/performance/`. Historical prototype archives are optional inputs to the report; ordinary CI does not run paid calls or desktop benchmarks. [Protocol, measurements and limits](performance-0.8.md) distinguish typing CPU, cold/warm state, nonstationary absolute timings, limited private-memory sampling and pending virtualization/provider work.

## Initial chat opening

The separate [initial-opening experiment](initial-open-0.8.md) uses `LUMEN_PERF_ROOT` to isolate its evidence and an explicit build-time `VITE_TRANSCRIPT_EXPERIMENT=1` flag for baseline/plain/virtual comparisons in one binary. Ordinary builds remove the virtual module and ignore experiment query parameters. `tests/initial-open-native.mjs`, `initial-open-stock.mjs` and `initial-open-report.mjs` collect the comparison, verify the normal candidate and emit sanitized aggregates. A secondary run writes a separate file and never overwrites the main series. Unset the build-time flag before normal builds; these experiments do not require any account or inference.

## Graph correctness checks

The post-0.7.1 graph corpus runs in normal Rust tests. `node tests/graph-import-oracles.mjs` checks TypeScript/Node resolution and runs a tiny existing-Godot GDScript fixture in TEMP, without model calls. After a debug build, `node tests/native-graph.mjs` exercises the real graph MCP across edits, rename, restart/persistent cache and deletion. `tests/graph-reliability-report.mjs` aggregates the local retained baseline/native evidence. [Scope and reproduction](graph-reliability-0.8.md).

## Turn waiting and CLI metadata

The metadata-only shutdown comparison freezes separate baseline/candidate Rust test executables and runs the installed-CLI probe in A/B/A/B blocks. `tests/metadata-shutdown-report.mjs` validates complete raw records and reports net metadata-call time including Windows job setup. `cargo test --manifest-path src-tauri/Cargo.toml --lib metadata_windows` runs real EOF, stubborn/orphaned child, cancellation, abort and concurrent-job isolation fixtures on Windows. [Fixed protocol and final confirmation](metadata-shutdown-0.8.md).

`tests/native-turn-latency.mjs` collects the separate turn-attribution matrix with offline CLI fixtures. Its default debug smoke/full reports are distinct from release evidence: set `LUMEN_LATENCY_EXE` to the frozen release built with the `.impeccable/turn-latency` overlay for the fixed 80-turn collection. Workspaces remain outside the original repository. `latency_probe::installed_cli_metadata_latency` is an ignored Rust test selected individually with an explicit `LUMEN_CLI_LATENCY_OUTPUT`; it launches installed CLI version/initialize/model-metadata commands only, in fresh configuration, without credential copying, login or model turns. `tests/turn-latency-report.mjs` requires both complete series and emits sanitized aggregates. Never run all ignored tests to select this probe. [Protocol, reproduction and remaining gaps](turn-latency-0.8.md).

## Live provider acceptance

`node tests/native-quality.mjs` exercises changed-check delivery exactly once, Claude/Claude/GPT handoff isolation, fresh quota precedence and same-provider independent review using offline CLI fixtures and real Windows IPC/storage. `node tests/quality-projects.mjs` checks the dedicated React/TypeScript and Godot C#/GDScript baselines against independent compile/runtime oracles; the reference corrections are not model output. Both fixtures intentionally start with behavioral defects.

`node tests/quality-preflight.mjs` checks existing native authentication/catalog metadata with no inference. `LUMEN_LIVE_QUALITY=1 node tests/quality-live.mjs` is a paid/quota-consuming, bounded experiment, never ordinary CI. It requires the exact preserved baseline binary, an already built candidate, installed dependencies/Godot/.NET and existing native accounts. It reserves each of three paired A/B slots per task/provider before inference in ignored `.impeccable/experiments/quality-live-budget.json`; failed/abandoned slots are never rerun. Do not delete this ledger to retry. Quota/authentication or harness prerequisite failures stop further spending for that provider. Temporary junctions reuse native authentication without copying credential files; cleanup removes only the junction. Read token usage only from exact native session IDs generated by this experiment. The [protocol](quality-experiments-0.8.md) defines scope and interpretation.

The live harness verifies reference projects in the actual external TEMP placement before inference. The checker canonicalizes Windows short paths before Vite startup. `node tests/quality-report.mjs` requires a concluded ledger and emits only aggregate/per-run check and usage data to `docs/data/quality-0.8-results.json`; raw native logs/account IDs and generated task paths are excluded. A changed final candidate binary cannot resume the old experiment. Partial usage and spent invalidated slots must remain visible.

`tests/live-native-smoke.mjs` requires explicit `LUMEN_LIVE_NATIVE=1`. It uses the owner's existing CLI account directories via temporary junctions without reading/copying credentials, disposable projects and bounded prompts. Normal probes cover questions, writes and native continuation. `LUMEN_LIVE_MOLI=1` covers browser calls/capture; Google print runners do not claim Claude/Codex native continuation. A diagnostic no-image control is available through `LUMEN_LIVE_MOLI_NO_IMAGE=1`.

Live graph/Orchestra/gameplay probes remain opt-in through the harness environment variables. Game acceptance uses an ignored isolated project, existing user-provided Godot/Graphify executables and no paid generation/deployment. The local Ollama browser probe is an ignored Rust test requiring an installed model; it never downloads one. Live calls consume the user's own existing quota and are not automatic CI work.

Protocol changes need metadata from the installed official binary, initialize/schema evidence and controlled regressions. Unknown callbacks must fail explicitly rather than gain broader approval. Never record credential files or unredacted private logs.

## Release procedure

1. Synchronize `package.json`, root package-lock entries, the Lumen Cargo package/lock entry and `tauri.conf.json`. Keep dependency versions intact.
2. Update current docs, release notes and the gallery; remove superseded work diaries, preserve historical changelogs and fix local links.
3. Run local checks and branch CI on Windows/macOS/Linux. Inspect the exact tracked diff and keep account data, artifacts, debug folders and purchased addons out of Git.
4. Fast-forward the default branch after verification. Create the annotated `v<version>` tag and push it.
5. The tag workflow builds all platforms, tests clean Windows installation and prepares a **draft** release with the matching Markdown changelog and SHA-256 manifest.
6. Verify all jobs, asset names, containers/checksums and portable resources, then publish the draft and mark it latest. A manual workflow run creates artifacts without publishing.

[Release history](release-notes/README.md) · [Architecture](architecture.md) · [Security](../SECURITY.md)
