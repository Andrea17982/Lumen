# Validation

## 0.6.0 release preflight (Windows, 2026-11-09)

| Check | Result | Scope |
| --- | --- | --- |
| `cargo test --locked` | 104 passed, 9 live tests excluded | Backend regression and controlled LSP/check fixtures |
| `cargo clippy --all-targets --locked -- -D warnings` | Pass | Library, binary and test lint coverage |
| `npm run check` | Production build and 59 tests passed | Frontend build and unit tests |
| `npx playwright test --workers=2` | 23 passed | Full Chromium UI suite; the initial 16-worker run had four timing failures, all passed with bounded concurrency |
| Formatting and `git diff --check` | Pass | Rust, frontend and patch formatting |

Task-ledger, diagnostics, validation and graph coverage/limits are documented in [Task quality](task-quality.md). Controlled protocol tests do not establish installed language-server or live provider interoperability. Release packaging and platform CI are separate from this local preflight.

## Godot integration (Windows, 2026-11-07)

Verified on Windows, 2026-11-07: Lumen started the project's `godot-pro` server (Godot MCP Pro 1.17.1, 187 tools) and read `get_project_info` from the open editor (Rodent Heroes, Godot 4.7.2, 360×640); with the editor closed it reported "not connected". Claude Code 2.1.314 and Codex 0.167.0-alpha.3.2 each called `get_project_info` in the project. Repository-map indexing, the `res://` check, the backend tests (270), simulation regressions (17) and the Godot .NET build ran through Lumen's check runner on an isolated clone.

## 0.5.0 local validation

| Check | Result | Scope |
| --- | --- | --- |
| `npm run build` | Pass | TypeScript and production frontend bundle |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | Pass | Rust application and targets, including screenshot MCP tools |
| `npm run format:check` | Pass | Prettier across frontend and browser fixtures |
| `cargo fmt --check` (app and Gatekeeper manifests) | Pass | Rust formatting |
| `git diff --check` | Pass | Whitespace and patch integrity |
| `npm run build:release` | Pass | Windows x64 NSIS and portable artifacts |
| Portable checksum | Pass | Portable SHA-256 matches `src-tauri/target/release/lumen.exe` |

Automated unit, Playwright and full Rust test suites were not run locally. The GitHub release workflow runs frontend and Rust tests before attaching release assets. Native checks for the screenshot MCP tool and settings remain open; macOS/Linux behavior is not established.

## 0.4.2

Evidence is separated by scope. Provider integrations still need acceptance testing with real accounts. The 0.4.0 evidence (native offline harness, live Orchestra run) is unchanged and is not repeated here.

| Check | Observed result | What it establishes |
| --- | --- | --- |
| TypeScript and Vite production build | Pass | Frontend type/build correctness |
| Vitest | 45 passing tests | Stream assembly, reasoning separation, usage snapshots and staleness, Orchestra state, live subtask output, Ollama titles, draft helpers |
| Playwright Chromium | 19 passing tests | Provider UI, Settings and Teamwork fields, Orchestra queue notice, working indicator, reasoning disclosure, usage table, composer badge, projects, workspace persistence |
| Rust tests | 49 passing, 3 ignored | Scheduler limits and retries, Orchestra recovery and process identity, approvals broker, Claude probe, Ollama list parsing, usage parsing and prompt helpers |
| Rust tests marked `#[ignore]` (run explicitly) | Pass on the owner's machine | Real Claude probe with the bridge, a real Haiku turn with an approved `Write` (the file was written), Ollama daemon call and missing-model refusal without download |
| `cargo clippy --tests -D warnings`, `cargo fmt --check` | Pass | Lint and formatting |
| Release build (Windows x64) | Pass | Portable executable reports 0.4.2 and matches the compiled binary |

The native harness replaces the CLIs with local fixtures and uses disposable data; it was not re-run for 0.4.2. The ignored tests call the installed CLIs with the owner's accounts, using a small model and a single approved file write.

Not established for 0.4.2: approve/reject buttons, countdown and working indicator in the native webview; Orchestra limits and Resume in a live run on a large project; Codex approvals; macOS and Linux behavior; clean-machine installs; code signing.

Earlier evidence for 0.4.0 remains in the 0.4.0 release notes.
