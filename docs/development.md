# Development

This checkout builds the local service and the Rust backend. The previous desktop client, its setup instructions, packaging workflow and UI checks are in the frontend archive.

Use Rust 1.90 or newer and Node.js 24 for the headless fixtures. On Linux, install the native libraries required by screen capture and keychain integration. See the CI workflow for the package list.

```sh
node scripts/prepare-moli.mjs
cargo build --manifest-path src-tauri/Cargo.toml -p lumen-daemon --locked
node tests/daemon-smoke.mjs
cargo test --manifest-path src-tauri/Cargo.toml --workspace --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
node tests/docs-check.mjs
```

The daemon smoke uses an isolated data directory and offline CLI fixtures. It exercises the daemon over authenticated IPC. Real-provider probes remain opt-in. See [architecture](architecture.md), [local service and recovery](local-service.md) and [validation](validation.md).

The headless TypeScript/Node and Godot graph-import oracles are in `tests/manual/graph-import-oracles.mjs`. The CLI metadata shutdown fixtures are in `lumen-core` under `metadata_windows`. Use synthetic data for ordinary checks. Tests that contact real providers must be run deliberately, with an authorized budget.
