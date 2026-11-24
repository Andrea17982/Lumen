# Lumen

Lumen is a local desktop app that orchestrates the AI coding agents the user already pays for: Claude Code, Codex, Antigravity, OpenCode, Cursor CLI, Ollama and the official Anthropic, OpenAI and Google APIs. It is MIT licensed, in alpha, and Windows x64 is the primary target.

Claude Code is the only agent developing this repository. Product code that integrates other agents is the product itself, so never remove it as clutter.

## State (Nov 2026)

- The backend is complete and released: the Rust workspace in `src-tauri/`, v0.12.2.
- The old frontend has been deleted. A new client is being written from scratch on branch `refactor/clean-slate`: `src/` (React) and `src-tauri/desktop/` (Tauri shell, outside the Cargo workspace until it is wired to the daemon). So far it holds the design tokens, the EN/IT catalog and a brand gallery.
- Roadmap, decisions and frontend research are in `notes/ROADMAP.md`. That file is private: read it before planning product work, never commit it and never link it from tracked files.

## Layout

| Path | What |
| --- | --- |
| `src-tauri/crates/lumen-core` | All domain logic: turns, routing, adapters, Orchestra, store, autonomy, Code Mode. Module map in `docs/architecture.md`. |
| `src-tauri/crates/lumen-daemon` | Headless service entry point; also `--lumen-code-mcp` MCP server mode. It owns processes, sessions and the queue. |
| `src-tauri/crates/lumen-ipc` | Authenticated local transport. Every client talks to the daemon only through this. |
| `src-tauri/crates/lumen-commands` | Typed command boundary. |
| `src-tauri/crates/lumen-gatekeeper` | Native approval broker for Claude and Codex, with protocol fixtures. |
| `src/` | New React client: `design/` (tokens, primitives on React Aria), `i18n/` (EN/IT catalog), `brand/`, `gallery/` (dev-only), `surfaces/`. Unit tests sit next to the code. |
| `src-tauri/desktop` | Tauri shell, outside the Cargo workspace until M2. |
| `tests/` | Node 24 headless harness: `daemon-smoke.mjs` (offline CLI fixtures, isolated data dir) and `docs-check.mjs`. `tests/manual/` is opt-in only. |
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
npx playwright test                                # gallery: axe, keyboard, 150% text, screenshots (Edge)
npx tauri build --no-bundle                        # desktop shell with the built client
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

## Frontend (to be built)

The planned stack is Tauri 2 + React 19 + TypeScript. Styling uses CSS Modules with semantic tokens and headless primitives. Animation uses CSS/WAAPI; add Motion only if it proves necessary.

- The UI must be minimal and Apple-like: few options, with complexity in the architecture. The brand is **Ink**: a stem-and-point mark, the lowercase serif wordmark "lumen", warm paper neutrals and an ember accent used sparingly. **Satin** (cool neutrals, indigo accent) is the only alternative style, chosen in Settings; both keep the same mark. Navigation is translucent (Mica, Liquid Glass), the conversation surface is opaque, fonts are system fonts.
- UI strings are in English and Italian.
- Long transcripts are the known performance risk. Isolate the streaming output, keep finished messages stable, and do not break native search, selection or accessibility.

## Git

- Work on feature branches and merge to `main` via PR.
- Never commit `notes/`, credentials, `.env*` or provider account directories.
- Keep only the current release notes in `docs/release-notes/`.
