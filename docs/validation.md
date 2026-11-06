# Validation — 0.3.1 (6 November 2026)

Evidence is separated by scope. Provider integrations still need acceptance testing with real accounts.

| Check | Observed result | What it establishes |
| --- | --- | --- |
| TypeScript and Vite production build | Pass | Frontend type/build correctness |
| Vitest | Pass | Stream assembly, authoritative replacement, diagnostic separation |
| Playwright Chromium | 9 passing tests | Provider UI, preview boundaries, theme, narrow layout, keyboard focus, projects, Settings errors, workspace persistence, account rename |
| Rust tests (Windows, macOS, Ubuntu in CI) | 20 passing tests | Routing, catalogs, permission flags, migrations, subprocesses, cancellation, rate-limit parsing, checkpoints and spill |
| Native Tauri/WebView2 offline harness | Pass | IPC, Both, cross-audit, rate-limit fallback, Rewind, self-healing, MCP switches, local API, queue/cancellation |
| Limited live run with the owner's accounts | Pass | Both, audit, a Claude edit followed by Rewind, local API request |
| Release workflow | Pass | Windows NSIS installer, portable exe, Apple silicon DMG |

The native harness replaces the CLIs with local fixtures and uses disposable data; it never authenticates or uses paid inference.

Not established: OAuth identity separation across two real accounts, token refresh, access to specific models, Linux GUI behavior, the macOS DMG on real hardware, startup/memory benchmarks, clean-machine installs, code signing, or an independent security audit.

Screenshots in `docs/images/` are captures of this implementation; `chat-fixture.png` uses the offline harness. See [Development](development.md) to reproduce the checks.
