# Development

You need Rust 1.90+ and Node.js 24. On Linux, also install the native libraries listed in `.github/workflows/ci.yml`.

```sh
node scripts/prepare-moli.mjs        # downloads and verifies the pinned Moli browser
cargo build --manifest-path src-tauri/Cargo.toml -p lumen-daemon --locked
node tests/daemon-smoke.mjs          # isolated data dir, offline CLI fixtures, real IPC
cargo test --manifest-path src-tauri/Cargo.toml --workspace --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
node tests/docs-check.mjs            # local links, anchors, crate versions, current release notes
```

The desktop client (React, in `src/`) and its shell (Tauri, in `src-tauri/desktop/`):

```sh
npm ci
npm run typecheck && npm test        # Vitest
npx playwright test                  # the client against the real debug daemon and offline fixtures
cargo clippy --manifest-path src-tauri/desktop/Cargo.toml --all-targets --locked -- -D warnings
npx tauri build --debug --no-bundle --features webdriver --config src-tauri/desktop/tauri.conf.json
npm run test:app                     # the real app over WebDriver; opens a window for about a minute (Windows)
npx tauri dev --config src-tauri/desktop/tauri.conf.json   # the app with hot reload
```

CI runs all of these on every push to `main` and on every pull request.

## Manual harnesses

The scripts in `tests/manual/` are opt-in and never run in CI. Their output goes to `test-results/`.

| Script | Purpose |
| --- | --- |
| `protocol-probe.mjs` | Metadata from installed CLIs only: no prompt, login or inference |
| `graph-import-oracles.mjs` | TS/JS and Godot import semantics for the graph corpus |
| `moli-probe.mjs` | Local feasibility check of the Moli browser |
| `live-routine-recognition.mjs` | Real Claude inference. Run only with an authorized budget |
| `final-performance.mjs`, `quality-evidence.mjs` | Release regression and evidence helpers |

Use synthetic data. Any test that contacts a real provider must be deliberate and budgeted.

## Releases

1. Bump the version in every crate's `Cargo.toml` and in `Cargo.lock`.
2. Write `docs/release-notes/<version>.md` and delete the previous one; GitHub Releases keeps the history.
3. Run `node tests/docs-check.mjs`. It fails if the crate versions disagree or the release notes are missing.
4. Build the Windows installer with `npx tauri build --config src-tauri/desktop/tauri.conf.json`. It builds the release daemon (`scripts/prepare-daemon.mjs`) and bundles it next to `Lumen.exe` with the Moli browser. The setup is in `src-tauri/desktop/target/release/bundle/nsis/`.
