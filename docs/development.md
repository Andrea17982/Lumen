# Development

You need Rust 1.90+ and Node.js 24. On Linux, also install the native libraries listed in `.github/workflows/ci.yml`.

## Build from source

From the repository root, install the frontend dependencies and the pinned Moli browser.

```sh
node scripts/prepare-moli.mjs
npm ci
```

```sh
npx tauri build --config src-tauri/desktop/tauri.conf.json
```

## Checks

Run the setup above before the checks.

```sh
cargo build --manifest-path src-tauri/Cargo.toml -p lumen-daemon --locked
node tests/daemon-smoke.mjs          # isolated data dir, offline CLI fixtures, real IPC
cargo test --manifest-path src-tauri/Cargo.toml --workspace --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
node tests/docs-check.mjs            # local links, anchors, crate versions, current release notes
```

The desktop client (React, in `src/`) and its shell (Tauri, in `src-tauri/desktop/`):

```sh
npm run typecheck && npm test        # Vitest
npx playwright test                  # the client against the real debug daemon and offline fixtures
cargo clippy --manifest-path src-tauri/desktop/Cargo.toml --all-targets --locked -- -D warnings
npx tauri build --debug --no-bundle --features webdriver --config src-tauri/desktop/tauri.conf.json
npm run test:app                     # the real app over WebDriver; opens a window for about a minute (Windows)
npx tauri dev --config src-tauri/desktop/tauri.conf.json   # the app with hot reload
```

CI runs the service checks on Windows, macOS and Linux, and the desktop checks on Windows. Use the last command for local hot reload.

## Manual harnesses

The scripts in `tests/manual/` are opt-in and never run in CI. Their output goes to `test-results/`.

| Script | Purpose |
| --- | --- |
| `protocol-probe.mjs` | Metadata from installed CLIs only: no prompt, login or inference |
| `moli-probe.mjs` | Local feasibility check of the Moli browser |
| `live-routine-recognition.mjs` | Real Claude inference. Run only with an authorized budget |
| `final-performance.mjs`, `quality-evidence.mjs` | Release regression and evidence helpers |

Use synthetic data. Any test that contacts a real provider must be deliberate and budgeted.

## Releases

1. Bump the version in every crate's `Cargo.toml` and in `Cargo.lock`.
2. Write `docs/release-notes/<version>.md` and delete the previous one; GitHub Releases keeps the history.
3. Run `node tests/docs-check.mjs`. It fails if the crate versions disagree or the release notes are missing.
4. Signed updates: the workflow signs the Windows setup and the macOS app archive when the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are set (never on pull requests). The public key is in `tauri.conf.json`. The tagged run also builds `latest.json` with `scripts/update-manifest.mjs`; upload it to the release with the installers, because installed copies read `releases/latest/download/latest.json`. Keep the private key outside the repository and backed up: without it, installed copies cannot receive updates.
5. Run the [source build](#build-from-source). It bundles the release daemon and Moli. On Windows, find the installer in `src-tauri/desktop/target/release/bundle/nsis/`. The [release workflow](../.github/workflows/release-builds.yml) defines the packages for each platform.
