# Development and verification

## Toolchain

Use Node 22.12+ / supported Node 24 and current stable Rust with the Windows MSVC toolchain. Install C++ Build Tools, Windows SDK and WebView2 per the [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). Resolved JavaScript and Rust versions are checked in via `package-lock.json` and `src-tauri/Cargo.lock`. Development initially used Node 24 and Rust 1.99.1; the package's minimum Rust field is not a claim that every future transitive dependency still supports an older toolchain.

```powershell
npm ci
npm run tauri dev
```

Use `npm run dev` for the honest browser UI preview; it never performs inference. For an installer run `npm run build:release`. The default Windows target is NSIS; the script copies the newest installer to `builds/latest/`. Tauri's original output is under `src-tauri/target/release/bundle/nsis/`. Signing and automatic updates are not configured.

## Checks

```powershell
npm run check
npx playwright install chromium
npm run test:e2e
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

Rust tests cover routing decisions/input validation, catalog parsing, rate-limit detection, checkpoints, spill, JSONL protocol fixtures, SQLite message deletion cascades, real subprocess stdin/stderr handling, nonzero exits and cancellation. Frontend unit tests cover stream reconciliation and diagnostic separation. Playwright tests cover first-run controls, provider advanced options, preview boundaries, theme persistence, keyboard focus containment/restoration and a narrow viewport. These tests do not log in, consume provider quotas or prove real-world model access.

For the Windows native offline smoke test, leave `npm run dev` running, build with `cargo build --manifest-path src-tauri/Cargo.toml`, then run `npm run test:native`. This launches the debug Tauri executable with a fresh temporary app-data directory, a PATH prefixed with `tests/fixtures/cli`, and a local WebView2 debugging port. It tests account metadata/status adapters, persistence, both provider event formats, review, parallel synthesis, queueing and cancellation. Fixtures never authenticate or call providers. Temporary test data is reported and retained for diagnosis. `LUMEN_TEST_DATA_DIR` is honored only in debug builds; release always uses normal application data. Do not expose the debug port externally or use this harness with real account data.

## Provider adapter contract

Claude uses a bidirectional stream-json connection with initialize, native question/permission callbacks and a persistent stdin writer. Codex uses app-server with initialized schema evidence, thread start/resume, per-turn cwd/permission settings and native tools. Autopilot preserves native tools while supported built-in browsing alternatives are routed to Moli. Restricted profiles retain explicit tool/sandbox limits; an additional universal OS jail is not implemented. Report exact installed CLI versions when filing compatibility issues. Google providers use their official print runners with the shared browser MCP adapter; Ollama uses its official loopback API and the same Moli dispatcher through a native-tool/JSON-action loop. See [Moli integration](moli.md) for packaging and live tests.

JSONL fixtures belong next to `providers::normalize`. Never record real credentials, private repositories or account-specific identifiers. `stderr` is drained but not displayed to avoid casually leaking raw auth diagnostics. This means some provider failures need investigation directly in the official CLI.

## Live acceptance (requires the user's own accounts)

`tests/live-native-smoke.mjs` is opt-in: set `LUMEN_LIVE_NATIVE=1` and `LUMEN_LIVE_PROVIDERS=gpt` (or a comma-separated supported set), then run it with the debug app built and Vite running. It creates temporary project/app-data folders, uses junctions to the official CLI account directories without reading/copying credentials, and removes those junctions on exit. It verifies a structured question, an actual file write and resume of the same native session.

Additional opt-in probes are `LUMEN_LIVE_GRAPH=1` and `LUMEN_LIVE_ORCHESTRA=1`. The isolated Rodent gameplay probe requires `LUMEN_LIVE_GAMEPLAY_ROOT` below this repository's `.codex/acceptance/`, `LUMEN_GODOT_EXE` and `LUMEN_GRAPHIFY_EXE`; it creates only the three standalone training-scene/check files. Keep those artifacts and the purchased addon out of version control. Live inference consumes the user's existing subscription quota; it is not part of CI.

`tests/release-smoke.mjs` verifies real initialization/tool discovery from `LUMEN_RELEASE_EXE`. Packaging runs it against the silently installed Windows executable on a disposable runner. The local build-copy helper regenerates `builds/latest/SHA256SUMS.txt`, including any renamed portable executable retained because the previous copy was running.

Use disposable folders and low-cost prompts. Complete official login for two distinct identities per provider and verify each status/billing identity through official tools without copying tokens. Test normal reply, follow-up context, wrong model, expired login, cancellation during queue/write-lock/streaming, connection loss, read-only denial and a small permitted edit. With both providers test writer/reviewer and parallel synthesis, including a failing second stage. Confirm no same-workspace simultaneous writes and no orphan process after ordinary cancellation.

Keep acceptance evidence redacted and distinguish it from fixtures. This task does not authorize automatically using a developer's existing account for paid inference.

## CI and release

`ci.yml` runs frontend checks and UI tests, Rust format/lint/tests on Windows, and Rust tests on macOS and Ubuntu, on every push and pull request. `release.yml` builds the Windows installer, the portable exe and the Apple silicon DMG; it runs on `v*` tags (and attaches the files plus `SHA256SUMS.txt` to a GitHub release) or manually from the Actions tab (artifacts only, no release). CI never contains provider credentials. Code signing is future work.

Before publishing, inspect the exact staged file list and scan for credentials, application databases and CLI homes. Screenshots must show the actual implementation and must not contain private account data. The donation address is intentionally public per the project owner's request.
