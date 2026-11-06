# Validation — 0.4.2

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
