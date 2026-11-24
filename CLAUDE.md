# Lumen

Lumen is a local desktop app that orchestrates the AI coding agents the user already pays for: Claude Code, Codex, Antigravity, OpenCode, Cursor CLI, Ollama and the official Anthropic, OpenAI and Google APIs. It is MIT licensed, in alpha, and Windows x64 is the primary target.

Claude Code is the only agent developing this repository. Product code that integrates other agents is the product itself, so never remove it as clutter.

## State (Nov 2026)

- Released as v0.12.5 (Windows x64 installer): the Rust workspace in `src-tauri/` and the new desktop client.
- The client was rewritten from scratch (milestones M0 to M10, all merged): `src/` (React) and `src-tauri/desktop/` (Tauri shell). It covers the design system, EN/IT, the daemon link with a synced cache, the conversation, attention and recovery, projects and sessions, accounts and quota, Orchestra as one switch, settings and extensions (sandboxed plugin panels), tray and notifications, the task branch bar, and the NSIS installer with the service as a sidecar.
- Roadmap, decisions and frontend research are in `notes/ROADMAP.md`. That file is private: read it before planning product work, never commit it and never link it from tracked files.

## Layout

| Path | What |
| --- | --- |
| `src-tauri/crates/lumen-core` | All domain logic: turns, routing, adapters, Orchestra, store, autonomy, Code Mode. Module map in `docs/architecture.md`. |
| `src-tauri/crates/lumen-daemon` | Headless service entry point; also `--lumen-code-mcp` MCP server mode. It owns processes, sessions and the queue. |
| `src-tauri/crates/lumen-ipc` | Authenticated local transport. Every client talks to the daemon only through this. |
| `src-tauri/crates/lumen-commands` | Typed command boundary. |
| `src-tauri/crates/lumen-gatekeeper` | Native approval broker for Claude and Codex, with protocol fixtures. |
| `src/` | New React client: `protocol/` (daemon types), `transport/` (Tauri, test bridge), `sync/` (snapshot + event cache, paged transcripts), `features/` (`conversation/`, `composer/`, `attention/`, `sessions/`, `settings/`, `orchestra/`, `worktree/`), `design/` (tokens, primitives on React Aria), `i18n/` (EN/IT catalog), `brand/`, `gallery/` (dev-only, `gallery.html`), `surfaces/`. Unit tests sit next to the code. |
| `src-tauri/desktop` | Tauri shell: `bridge.rs` starts or joins the daemon, relays `__events` on `lumen:event` and passes commands through `lumen_ipc::Client::call`. Kept outside the Cargo workspace so the Rust CI needs no WebView or `dist/`; CI checks it in the Windows client job. The `webdriver` feature embeds a WebDriver server for tests and refuses to compile in release. |
| `tests/` | Node 24 headless harness: `daemon-smoke.mjs` (offline CLI fixtures, isolated data dir) and `docs-check.mjs`. `ui/` holds the Playwright specs (gallery, conversation, attention, sessions, accounts, Orchestra, settings and extensions, task worktree, crash recovery, 5,000-message budget), `app/` drives the real app over WebDriver (shell recovery, plugin panel cannot reach IPC, UI Automation reach of a long transcript), `support/` has the test-only HTTP bridge, a minimal W3C WebDriver client, the app launcher and a seeded transcript generator. `tests/manual/transcript-5k.mjs` opens the app on 5,000 messages for screen reader checks. `tests/manual/` is opt-in only. |
| `plugins/` | Lumen plugins (Godot, examples). |
| `scripts/` | `prepare-moli.mjs` (bundled browser) and `prepare-daemon.mjs`. |
| `docs/` | Public user docs. Keep them describing the shipped product. |

## Commands

Run from the repo root. Use PowerShell or Bash; the paths are the same.

```sh
node scripts/prepare-moli.mjs                      # once, before the first build
cargo build --manifest-path src-tauri/Cargo.toml -p lumen-daemon --locked
node tests/daemon-smoke.mjs
cargo test --manifest-path src-tauri/Cargo.toml --workspace --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
node tests/docs-check.mjs                          # after any doc or version change
npm ci                                             # client dependencies
npm run typecheck && npm test                      # client typecheck and unit tests (Vitest)
npx playwright test                                # gallery (axe, keyboard, 150% text, screenshots) and the daemon crash test; needs the debug daemon built
cargo clippy --manifest-path src-tauri/desktop/Cargo.toml --all-targets --locked -- -D warnings   # shell; also cargo test and fmt
npx tauri build --no-bundle --config src-tauri/desktop/tauri.conf.json   # release shell with the built client
npx tauri build --debug --no-bundle --features webdriver --config src-tauri/desktop/tauri.conf.json
npm run test:app                                   # real app over WebDriver; opens a window for ~1.5 min (Windows)
```

CI (`.github/workflows/ci.yml`) runs the Rust and docs commands on Windows, macOS and Linux, and the client commands on Windows. Before calling work done, run the subset that covers what you touched. For Rust changes that is at least `cargo test` and `clippy` on the affected crate (`-p <crate>`).

## Claude tooling (`.claude/`)

- **Hook:** `rustfmt` runs on every `.rs` file after Edit/Write; a syntax error is reported back.
- **Review agents** (adapted from ECC, MIT, see `.claude/ECC-LICENSE`): use `rust-reviewer` after non-trivial Rust changes, `security-reviewer` for the security-sensitive areas below, and `silent-failure-hunter` for error handling and recovery paths. Their findings are input: verify them before acting.
- **Skills:** `verification-loop` before declaring a feature done or opening a PR, `rust-testing` when writing tests, `security-review` for input, auth and secrets, `moli` for all web browsing (bundled headless browser, no Chrome). Do not add the dependencies they suggest (rstest, mockall, proptest, criterion) without the owner's approval.

## Rules

- **Daemon is the source of truth.** Clients render state and send commands through `lumen-ipc`. They never own execution, approvals or protocol decisions.
- **Official interfaces only.** No inference proxy, no token extraction, no provider OAuth. Authentication stays in each official CLI; API keys go in the OS keychain.
- **Never replay observed work.** After quota, Stop, crash or handoff, keep partial output and effects. If state is uncertain, stop or offer an explicit recovery.
- **No fake success.** A build or test counts as passed only with a tool result or artifact. State verification that was not run.
- **Declared capabilities.** Each adapter states what it supports. Never claim parity that the official interface does not provide.
- **Tests use fixtures, not live providers.** Real-provider probes (`tests/manual/`) need explicit approval and a budget from the owner.
- **Minimal and measured.** Reuse existing modules. Measure before optimizing. Add no speculative abstractions or dependencies.
- **Security-sensitive areas:** `autonomy.rs`, `approvals*.rs`, `lumen-gatekeeper`, `lumen-ipc`, and anything remote or network-facing. Review these changes for permission bypass and prompt injection.

## Frontend

The planned stack is Tauri 2 + React 19 + TypeScript. Styling uses CSS Modules with semantic tokens and headless primitives. Animation uses CSS/WAAPI; add Motion only if it proves necessary.

- The UI must be minimal and Apple-like: few options, with complexity in the architecture. The brand is **Ink**: a stem-and-point mark, the lowercase serif wordmark "lumen", warm paper neutrals and an ember accent used sparingly. **Satin** (cool neutrals, indigo accent) is the only alternative style, chosen in Settings; both keep the same mark. Navigation is translucent (Mica, Liquid Glass), the conversation surface is opaque, fonts are system fonts.
- UI strings are in English and Italian.
- Long transcripts are the known performance risk. Isolate the streaming output, keep finished messages stable, and do not break native search, selection or accessibility.

## Git

- Work on feature branches and merge to `main` via PR.
- Never commit `notes/`, credentials, `.env*` or provider account directories.
- Keep only the current release notes in `docs/release-notes/`.
