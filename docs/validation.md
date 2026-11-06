# Validation — 0.4.0 (6 November 2026)

Evidence is separated by scope. Provider integrations still need acceptance testing with real accounts.

| Check | Observed result | What it establishes |
| --- | --- | --- |
| TypeScript and Vite production build | Pass | Frontend type/build correctness |
| Vitest | 3 passing tests | Stream assembly, authoritative replacement, diagnostic separation, Orchestra state and live subtask output |
| Playwright Chromium | 12 passing tests | Provider UI, preview boundaries, theme, narrow layout, keyboard focus, projects, Settings sections and Simple/Advanced, workspace persistence, account rename, Orchestra cards and composer badge |
| Rust tests (Windows, macOS, Ubuntu in CI) | 30 passing tests | Routing, catalogs, permission flags, migrations, subprocesses, cancellation, rate-limit parsing, checkpoints, spill, Orchestra plan parsing/assignment/edits, Ollama list parsing, close-to-tray decisions |
| Native Tauri/WebView2 offline harness | Pass | IPC, Both, cross-audit, rate-limit fallback, Rewind, self-healing, MCP switches, local API, queue/cancellation, Ollama status/catalog, Orchestra questions → plan → parallel worktrees → integration, close-to-tray and Quit |
| Limited live run with the owner's accounts | 0.3.1 only | Both, audit, a Claude edit followed by Rewind, local API request. Orchestra and Ollama are not yet tested live |
| Release workflow | Pass | Windows NSIS installer, portable exe, Apple silicon DMG |

The native harness replaces the CLIs with local fixtures and uses disposable data; it never authenticates or uses paid inference.

Not established: Orchestra with real models (plan quality, merge conflicts on real code), Ollama inference with a real model, the login item on macOS, OAuth identity separation across two real accounts, token refresh, access to specific models, Linux GUI behavior, the macOS DMG on real hardware, startup/memory benchmarks, clean-machine installs, code signing, or an independent security audit.

Screenshots in `docs/images/` are captures of this implementation; `chat-fixture.png` uses the offline harness. See [Development](development.md) to reproduce the checks.
