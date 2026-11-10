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

## Live provider acceptance

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
