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

Rust tests cover routing decisions/input validation, JSONL protocol fixtures, SQLite message deletion cascades, real subprocess stdin/stderr handling, nonzero exits and cancellation. Frontend unit tests cover stream reconciliation and diagnostic separation. Playwright tests cover first-run controls, provider advanced options, preview boundaries, theme persistence, keyboard focus containment/restoration and a narrow viewport. These tests do not log in, consume provider quotas or prove real-world model access.

For the Windows native offline smoke test, leave `npm run dev` running, build with `cargo build --manifest-path src-tauri/Cargo.toml`, then run `npm run test:native`. This launches the debug Tauri executable with a fresh temporary app-data directory, a PATH prefixed with `tests/fixtures/cli`, and a local WebView2 debugging port. It tests account metadata/status adapters, persistence, both provider event formats, review, parallel synthesis, queueing and cancellation. Fixtures never authenticate or call providers. Temporary test data is reported and retained for diagnosis. `LUMEN_TEST_DATA_DIR` is honored only in debug builds; release always uses normal application data. Do not expose the debug port externally or use this harness with real account data.

## Provider adapter contract

Claude: `auth login/status`, `--print --verbose --output-format stream-json --include-partial-messages`, effort/model overrides, `dontAsk`, explicit tools, strict MCP config and `--restricted` to confine file tools to working directories. Codex: `login/status`, `exec --json`, stdin prompt, sandbox override, approval policy and reasoning effort. Older versions may reject these flags, especially Claude restricted mode. Report exact CLI versions when filing a compatibility issue.

JSONL fixtures belong next to `providers::normalize`. Never record real credentials, private repositories or account-specific identifiers. `stderr` is drained but not displayed to avoid casually leaking raw auth diagnostics. This means some provider failures need investigation directly in the official CLI.

## Live acceptance (requires the user's own accounts)

Use disposable folders and low-cost prompts. Complete official login for two distinct identities per provider and verify each status/billing identity through official tools without copying tokens. Test normal reply, follow-up context, wrong model, expired login, cancellation during queue/write-lock/streaming, connection loss, read-only denial and a small permitted edit. With both providers test writer/reviewer and parallel synthesis, including a failing second stage. Confirm no same-workspace simultaneous writes and no orphan process after ordinary cancellation.

Keep acceptance evidence redacted and distinguish it from fixtures. This task does not authorize automatically using a developer's existing account for paid inference.

## CI and release

GitHub Actions runs frontend checks, UI tests and Windows Rust checks on pushes/PRs. Tags beginning with `v` run the installer build and attach the installer plus SHA-256 checksum to a GitHub release. CI never contains real provider credentials. Releases are marked prerelease for alpha tags; code signing remains future work. Local first-release assets can also be uploaded using `gh release create` after the same checks pass.

Before publishing, inspect the exact staged file list and scan for credentials, application databases and CLI homes. Screenshots must show the actual implementation and must not contain private account data. The donation address is intentionally public per the project owner's request.

## 0.2 checks

Official metadata discovery is tested with fixture implementations of the Codex JSON-RPC initialization/model-list protocol and Claude initialization control response. The native harness verifies workspace-folder binding and the actual working directory inside its CLI fixture (including paths containing spaces), model-specific effort choices, both account rename entry points, and an offline Autopilot turn. Rust tests additionally cover legacy database upgrade and explicit permission-profile flag mapping. Short-lived official CLI probes confirmed that both real installed CLIs return model/effort metadata from fresh temporary profiles without submitting a turn. These probes establish protocol compatibility, not account entitlements or paid inference.
