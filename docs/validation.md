# Validation, release scope and known limits — 0.7

This is the canonical acceptance record. Implementation, automated proof and environment-dependent live testing are distinct. The implementation roadmap and temporary development diary have been retired after consolidation here.

## Closed implementation scope

| Area | Delivered behavior |
| --- | --- |
| Native sessions | Direct default, preserved provider/model, Claude bidirectional control and Codex app-server, native continuation/questions/child output where supported |
| Autonomy | No duplicate plan approval in Autopilot/already approved execution; actual permissions and explicit working-folder changes reach ordinary and Orchestra turns |
| Task quality | Durable source-linked decisions, real tokenizer, ordered/coalesced deltas, graph/impact context, diagnostics and affected checks |
| Project tools | Inspector, layout canvas, source/resource diffs, manual Both merge, static worktree comparisons, opt-in Godot actions/capture |
| Reliability | Versioned transactional migrations, notification-area continuity and explicit interrupted-run recovery |
| Browser | Bundled pinned Moli, shared native MCP adapters, isolated contexts, permissions/cancellation, images and local Ollama tool/result loop |
| Distribution | Windows installer/portable ZIP, universal macOS DMG, Linux DEB/AppImage and checksum/release automation |

**Removed by the owner:** signing, Telegram, internal database encryption, a separate daemon and an additional common OS path jail. Agreed alternatives are unsigned packages, OS/disk security, native provider controls and tray continuity.

**Conditional proposals evaluated, not shipped:** full transcript virtualization, Rust Markdown/ANSI rendering, warm CLI pools and tuple-based metadata. The measurements below did not establish a need/reliable model-generation benefit that justified replacing the existing native formats/lifecycle. These are not presented as implemented functions or hidden release blockers.

## Automated preflight

The 0.7 code baseline passes 135 Rust tests, 60 TypeScript tests and 29 Playwright tests, plus rustfmt, Prettier and Clippy with warnings denied. Thirteen environment/account-dependent Rust probes are opt-in; the real local Ollama browser probe was also executed separately. Fresh release checks and package results are available from [CI](https://github.com/Andrea17982/Lumen/actions/workflows/ci.yml), [packaging](https://github.com/Andrea17982/Lumen/actions/workflows/release.yml) and the [0.7.0 release](https://github.com/Andrea17982/Lumen/releases/tag/v0.7.0).

Before the version bump, functional source `3ee4635` passed [all-platform CI](https://github.com/Andrea17982/Lumen/actions/runs/42843169200) and [all-platform packaging](https://github.com/Andrea17982/Lumen/actions/runs/42843169596). The 0.7 release repeats the required checks. Builds do not establish every platform's GUI/login/display behavior.

The Windows native offline harness covers streaming, actual question/answer UI, writes/read-only/Both/Orchestra, queue/cancel/fallback, stale merge rejection, inspector/layout, Godot opt-in/wrong-project rejection, shortcut registration and tray close/Quit. It uses isolated data and controlled CLIs/local Ollama HTTP peers, not paid inference.

Real bundled Moli tests cover fetch, page JavaScript, fill/click/evaluate, extraction, PNGs, context reuse, worker isolation, scoped permissions, authentication, cancellation and Antigravity transport adaptation. Ollama fixtures exercise both native function calls and JSON-action fallback, each consuming a real Moli result in the next model request.

Packaging checks verify a clean Windows install, installed graph/browser MCP discovery and the bundled Moli executable. The local portable ZIP was independently extracted and exercised against a real local page; its SHA-256 manifest matched all local artifacts. Non-Windows packages receive automated builds/tests, not a claim of manual desktop acceptance.

## Observed live and project acceptance

| Probe | Observed result | Limit |
| --- | --- | --- |
| Codex native session | Structured question, actual file write and same-thread continuation | One installed CLI/account configuration, not every version/identity |
| Codex graph | Actual configured `find_symbol` call returned the source location | Not every graph pattern/language |
| Codex Orchestra | Autopilot, concurrency 1/total 2, two real writes integrated without another approval | Not every quota/crash/restart combination |
| Codex Moli | Navigate, click, extract, PNG and native resume of the same tab; about 27 s for two turns | Bounded local browser fixture |
| Antigravity Moli | Flash High completed navigation/click/extract/capture/final answer in about 29 s | Inline MCP images stalled earlier trials; saved PNG/native-viewer route is used instead |
| Local Ollama | Installed llama3.2 called Moli and returned an unpredictable value from the page | No model downloaded or remote inference; not every local model |
| Installed language servers | Actual Rust, TypeScript, C# and Godot type errors observed | Selected versions/roots, not complete compilation coverage |
| Godot/project loop | Real opt-in Discover/Play/Stop/Reload/capture, compiler links and manifest-corruption check in an isolated project | No original-project edit, deployment or purchased-asset redistribution |
| Offline gameplay fixture | Generated training scene/check passed real Godot button/state/reset verification and visual review | Desktop proof; no physical-device or production-balance qualification |

Claude initialization was observed without a model turn. Latest Claude inference remains explicitly deferred after quota exhaustion. Gemini CLI 0.67.0 was installed and version/help checked; the owner completes native login. Neither is converted into a fabricated 0.7 live pass.

## Performance evidence

- A Windows native 1,000-message synthetic transcript loaded in about 0.5–0.6 s. Synthetic jumps produced an isolated roughly 0.33–0.36 s worst frame interval; this is not a universal smoothness claim.
- One warm debug/Vite/WebView2 startup reached the welcome UI in about 1.76 s. Summed process working sets counted shared pages repeatedly; no cold-start/private-memory release guarantee is inferred.
- Fixed-schema tuples reduced local tokens by about 32% on synthetic owned metadata, but did not prove reliable model generation. Native/keyed formats remain in use.
- The Moli feasibility fixture measured roughly 1.5× episode throughput against Chromium for one small local workload. It did not measure general model intelligence, complete conversation speed or browser-wide compatibility. [Method and limits](moli.md).

## Deferred acceptance, explicitly retained

- Claude live inference after quota and the owner's Gemini native login/model turn.
- Broader multi-account, CLI-version, quota/restart and repeated Resume matrices.
- Native macOS/Linux login, desktop capture, accessibility/screen-reader, window/system shortcut and clean-machine testing with real testers.
- Alternate uninstalled editor caret-position checks, physical devices and non-Windows Godot/editor behavior.
- Moli website/CAPTCHA/rendering coverage beyond controlled fixtures. Canvas/WebGL/pixel parity are not guaranteed.

These are account/hardware/coverage limits, not claims of missing core wiring or promises that fixtures proved them. Lumen 0.7 remains an alpha. [Security](../SECURITY.md) · [Changelog](release-notes/0.7.0.md).
