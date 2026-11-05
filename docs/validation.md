# Alpha validation — 5 November 2026

Evidence for the initial alpha is deliberately separated by scope. Provider integrations still need real-account acceptance testing.

| Check | Observed result | What it establishes |
| --- | --- | --- |
| TypeScript and Vite production build | Pass | Frontend type/build correctness |
| Vitest | 2 passing tests | Stream assembly, authoritative replacement, diagnostic separation |
| Playwright Chromium | 5 passing tests | First-run/provider UI, preview boundaries, theme persistence, narrow layout, keyboard focus, search preserving active session |
| Rust unit/process tests on Windows | 7 passing tests | Routing, input checks, protocol fixtures, SQLite cascade, real subprocess stdin/stderr, nonzero exit and cancellation |
| Rustfmt and Clippy with warnings denied | Pass | Formatting and lint checks |
| Native Tauri/WebView2 offline harness | Pass | IPC, isolated test storage, account/status adapters, both event formats, writer/reviewer, parallel synthesis, queue/cancellation and populated UI |
| Independent UI finish review | Ship within reviewed scope | Inspected desktop light, dark settings, settled narrow view, native fixture chat and activity; keyboard findings corrected |
| Windows x64 NSIS packaging | Pass | Installer generation; not a clean-machine installation test |

The native harness substitutes local CLI fixtures before launching the app and uses disposable test data. It never authenticates with a provider or uses paid inference. Its sample text explicitly says it is an offline fixture. The test found and led to correction of a Windows launcher problem with paths containing spaces; the re-run passed.

Not established: real OAuth identity separation across two accounts, token refresh, access to any specific model, actual AI reply quality, real provider tool boundaries, Linux behavior, startup/memory benchmarks, clean-machine installer behavior, code-signing trust or an independent security audit. CI results are available in the repository's Actions tab; local results above do not predict a remote run.

Screenshots in `docs/images/` are actual captures of this implementation. `chat-fixture.png` is a native WebView2 capture using the offline harness, while the empty desktop/dark captures use the browser UI preview. See [Development](development.md) to reproduce the checks and [Roadmap](roadmap.md) for remaining release gates.
