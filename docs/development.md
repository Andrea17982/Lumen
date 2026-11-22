# Development, tests and releases

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

`node tests/native-shared-integrations.mjs` adds isolated Windows acceptance for discovered installations, stdio relay/process-tree cleanup, native account RPC without model turns, original skill resources, restricted catalogs, unsupported APIs and a Claude → ChatGPT-account tool call with consent shown by Lumen itself. It uses offline fixtures, never real credentials or inference. Run it serially with all other port-1420 suites. [Scope and remaining requirements](integrations.md).

After a debug Cargo build, `node tests/native-connectors.mjs` checks MCP form/URL consent, typed validation/retry, decline, resolution/Stop expiry and Claude Ask approvals in real Windows WebView2/IPC with offline CLI fixtures. It never signs in or opens the fixture OAuth URL. Run it separately from Playwright and other native harnesses: they share port 1420, and an owning harness closing its server interrupts the others. See [shared integrations](integrations.md).

```sh
npm run check
npm run format:check
npx playwright install chromium
npx playwright test --workers 2
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
node scripts/prepare-daemon.mjs --debug
node tests/daemon-smoke.mjs
cargo test --manifest-path src-tauri/Cargo.toml --workspace --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
node tests/docs-check.mjs
```

Playwright reuses any server already on its port. When another checkout's dev server holds 1420, set `LUMEN_E2E_PORT` (for example `LUMEN_E2E_PORT=1437 npx playwright test`) so the run serves this checkout. `tests/transcript.spec.ts` covers long transcripts offline with fixture data: order, variable-height rows, streaming without remounting earlier rows, keyboard expand/collapse, bounded output and search. `node tests/transcript-performance.mjs <preview-url> <label>` times opening, streaming and scrolling a 2,000-message fixture against a `vite build` + `vite preview` of each version.

The docs check validates local Markdown links/images, the current release notes and version consistency across application manifests. Rust tests use real subprocesses, disposable Git projects, local protocol peers and the bundled Moli fixture. Opt-in live tests are excluded from ordinary CI; [Validation](validation.md) distinguishes these scopes.

## Native Windows and screenshots

Keep Vite running, build `node scripts/prepare-daemon.mjs --debug` and `cargo build --manifest-path src-tauri/Cargo.toml --workspace`, then run `npm run test:native`. The harness uses fresh temporary app data, controlled CLI fixtures, a local Ollama API simulator and loopback WebView2 debugging. It does not authenticate or call remote inference. `LUMEN_TEST_DATA_DIR` is debug-only; release uses normal app data.

`node tests/capture-release.mjs` captures the documentation gallery from the native Windows application with isolated demonstration data. It never opens real account directories. Screenshots are actual UI captures; captions must disclose fixture data and distinguish them from live acceptance evidence. Close the owned app/server when finished. Do not expose test/debugging ports to the network.

`node tests/native-composer.mjs` is a focused Windows/debug acceptance harness after the same Cargo build. It starts a dev server only when one is absent, uses fresh SQLite/account/WebView2 data and offline Claude fixtures, and stops its owned processes. It checks per-chat text/attachments, immediate pending delivery, duplicate-send protection, delayed acceptance across chat switches, rejected-send recovery, uncertain readback, new-chat creation and delayed Compact. A page-scoped dev-module transport seam introduces controlled delays/failures without modifying Tauri internals or production source; accepted turns still use real IPC/storage/CLI fixtures. Drafts currently survive chat switches in the same window, not reload/restart. Results and light/dark pending-state screenshots are written under `.impeccable/review/`; this is not a release performance baseline or live model comparison.

`node tests/native-execution.mjs` uses the shared `tests/native-harness.mjs` fixture for focused execution-observability acceptance: actual question/answer, provider return without tools, explicit tool lifecycle, provider failure, a real 30-second silence, read-only refresh/cancel, delayed refresh after navigation, internal Orchestra trace persistence and Italian/dark rendering after reload. Run native harnesses serially so a dev server owned by one harness is not closed while another uses it. They use isolated data and offline CLIs, without live inference. Test shutdown tolerates only the expected owned-webview-close acknowledgment race. Results/captures are under `.impeccable/review/native-execution-results.json` and `execution-*.png`.

## Measurement harnesses

These harnesses produce local evidence for performance or correctness changes. They never run in CI, use isolated benchmark identities/profiles and keep raw samples, fixtures, snapshots and captures under the ignored `.impeccable/` folder. Run them serially, away from unrelated CPU-intensive work, and keep failed or inconclusive observations. Node with `node:sqlite` is needed for native fixture database writes.

- **Long chats and typing:** `node tests/performance-prepare.mjs` prepares a benchmark profile; build with `npx tauri build --no-bundle --config .impeccable/performance/config.json` and freeze baseline/candidate executables together with their matching `lumen-daemon.exe` and resources. Do not use the ordinary release GUI for an isolated benchmark: build with the prepared performance identifier. `node tests/performance-native.mjs baseline|candidate|confirmation` collects 20 repetitions per 100/1,000/5,000-message workload with no accounts or inference; `performance-scroll.mjs` and `performance-acceptance.mjs` cover scrolling and content/action checks on the exact candidate hash.
- **Initial chat opening:** `tests/initial-open-native.mjs` and `initial-open-stock.mjs`, isolated with `LUMEN_PERF_ROOT`. The excluded virtual-window prototype compiles only with `VITE_TRANSCRIPT_EXPERIMENT=1`; unset it before normal builds.
- **Graph correctness:** the import corpus runs in normal Rust tests. `node tests/graph-import-oracles.mjs` checks TypeScript/Node resolution and a tiny Godot GDScript fixture in TEMP (override Godot with `LUMEN_GODOT_EXE`); after a debug build, `node tests/native-graph.mjs` exercises the real graph MCP across edits, rename, restart and deletion.
- **CLI metadata and turn waiting:** `cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --lib metadata_windows` runs real Windows Job Object shutdown fixtures. `latency_probe::installed_cli_metadata_latency` is an ignored test selected individually with `LUMEN_CLI_LATENCY_OUTPUT`; it launches installed CLI version/initialize/model-metadata commands only, without login or model turns. Never run all ignored tests to select it. `tests/native-turn-latency.mjs` collects turn-phase attribution with offline CLI fixtures (`LUMEN_LATENCY_EXE` selects a frozen release).

Compare against the same hardware and a frozen baseline.

## Adapter corpus

After `npm run build`, a debug build and a production preview on port 1420, run `node tests/adapter-corpus.mjs`. It serializes the shared semantic corpus across native protocol-specific harnesses: catalogs, terminal results, permissions, tool-result round trips, partial output, quota, cancellation, continuation, identity/workspace scope and selector layout. The receipt is `.codex/verification/09-adapter-corpus.json`. Its API suite uses fake UUID-owned OS-keychain entries and removes them afterward; no real key or paid endpoint is used.

`node tests/native-opencode-installed.mjs` is a separate installed-runtime interoperability test. It uses the installed OpenCode 2.2.7 with a temporary project/custom loopback provider; a native provider policy denies every other provider. It verifies all four permission profiles, actual blocked shell/external writes and ACP Allow once/Deny. It is not a live-model quality benchmark.

API debug fixtures require `LUMEN_TEST_DATA_DIR` plus a root-only HTTP loopback `LUMEN_TEST_API_BASE`. Release builds ignore the override. Never launch a release with this environment expecting test-data isolation; native release verification uses package inspection, not the real user database. API costing tests include persistent/concurrent reservations, Auto revocation, uncertain cancellation and host-command tree cancellation.

## Provider acceptance

After the debug build, `node tests/native-cursor.mjs` exercises Cursor catalog/storage, a single native profile, long UTF-8 stdin, native continuation, terminal success/error/truncation, partial output, quota resumption, inherited API-key removal, permission rejection, explicit Auto instructions and owned-tree cancellation. It uses offline fixtures and no real Cursor login/inference. `node tests/native-081-continuity.mjs` retains the prior continuation/OpenCode regressions. Run these and `node tests/native-menu-layout.mjs` serially against the production preview when validating a release.


`node tests/native-quality.mjs` exercises changed-check delivery, Claude/Claude/GPT handoff isolation, quota precedence and same-provider review with offline CLI fixtures and real Windows IPC/storage. `node tests/quality-projects.mjs` checks the dedicated React/TypeScript and Godot C#/GDScript reference projects against independent compile/runtime oracles. `node tests/quality-preflight.mjs` checks existing native authentication/catalog metadata without inference.

Releases rely on fixtures, not live quality benchmarks. Live runners are opt-in and use real inference: `tests/live-routine-recognition.mjs` (memory and schedule recognition; `CLAUDE_CONFIG_DIR` selects a Lumen account slot, `CASES` limits the cases), `tests/live-routing-acceptance.mjs` (`LUMEN_LIVE_ROUTING=1`, plus `LUMEN_CONTROLLED_QUOTA=1`) and `tests/native-idle-acceptance.mjs` (`LUMEN_LIVE_IDLE=1`). `tests/quality-live.mjs` must not be run without an explicit new budget.

`tests/live-native-smoke.mjs` requires explicit `LUMEN_LIVE_NATIVE=1`. It uses the owner's existing CLI account directories via temporary junctions without reading/copying credentials, disposable projects and bounded prompts. Normal probes cover questions, writes and native continuation. `LUMEN_LIVE_MOLI=1` covers browser calls/capture; Google print runners do not claim Claude/Codex native continuation. A diagnostic no-image control is available through `LUMEN_LIVE_MOLI_NO_IMAGE=1`.

Live graph/Orchestra/gameplay probes remain opt-in through the harness environment variables. Game acceptance uses an ignored isolated project, existing user-provided Godot/Graphify executables and no paid generation/deployment. The local Ollama browser probe is an ignored Rust test requiring an installed model; it never downloads one. Live calls consume the user's own existing quota and are not automatic CI work.

Protocol changes need metadata from the installed official binary, initialize/schema evidence and controlled regressions. Unknown callbacks must fail explicitly rather than gain broader approval. Never record credential files or unredacted private logs.

## Release procedure

1. Synchronize `package.json`, root package-lock entries, the Lumen Cargo package/lock entry and `tauri.conf.json`. Keep dependency versions intact.
2. Update current docs, the release notes for this version and the gallery; remove superseded material (earlier changelogs live in GitHub Releases) and fix local links.
3. Run local checks and branch CI on Windows/macOS/Linux. Inspect the exact tracked diff and keep account data, artifacts, debug folders and purchased addons out of Git.
4. Fast-forward the default branch after verification. Create the annotated `v<version>` tag and push it.
5. The tag workflow builds all platforms, tests clean Windows installation and prepares a **draft** release with the matching Markdown changelog and SHA-256 manifest.
6. Verify all jobs, asset names, containers/checksums and portable resources, then publish the draft and mark it latest. A manual workflow run creates artifacts without publishing.

[Release history](release-notes/README.md) · [Architecture](architecture.md) · [Security](../SECURITY.md)

Local Windows `npm run build:release` uses release optimization level 3, LTO, one codegen unit, stripping and `target-cpu=native` (including AVX2 when supported). Its binaries require a CPU compatible with the build machine. CI continues to build portable target baselines with `npm run tauri build`; it does not inherit local CPU flags.

## Service checks

`prepare-daemon.mjs` builds the headless service before Tauri dev/build and prepares the target-suffixed sidecar, including both architectures for a universal macOS build. Frontend-only builds remain independent of Rust. `daemon-smoke.mjs` exercises authenticated headless startup, exclusivity, framing/version checks, concurrent commands, update refusal/drain, restart and owned process cleanup. `native-daemon.mjs` exercises complete GUI restarts during a turn, native question and Orchestra, persistent quota cancellation, and a hard service crash beside an unrelated process. These use isolated directories and offline fixture protocols. Feature suites (`native-memory-routines.mjs`, `native-quality.mjs`, `native-execution.mjs`) drive the real GUI and daemon in the same isolated way. Native harness cleanup stops its own daemon explicitly; closing a fixture GUI alone intentionally leaves it running. See [service operations](local-service.md).

Cargo defaults to the GUI package so Tauri does not rebuild a second, differently unified daemon after sidecar preparation. Backend checks use `--workspace` (or `-p lumen-core` for one domain probe). The sidecar is built explicitly before the GUI.
