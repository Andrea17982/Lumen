# Validation, release scope and known limits — 0.7

The completed checkpoints below are included in **0.7.1 — Performance Update**. Earlier sections retain their 0.7.0/checkpoint dates, versions and hashes as historical evidence; the remaining 0.8 roadmap is not closed. [0.7.1 release notes](release-notes/0.7.1.md).

## 0.7.1 metadata shutdown acceptance

The fixed installed-CLI A/B/A/B experiment completes 160 metadata calls plus 160 version probes, without inference. Complete-call medians decrease 37.2% for Claude and 57.4% for Codex, including roughly 42 ms extra supervised-spawn setup. Cleanup medians fall from 428/423 to 44/18 ms. A separate final-source/version confirmation completes another 20 samples per provider: 30.2%/50.2% lower complete-call medians against the preserved baseline. All raw blocks, first samples, hashes, model counts, p95 and setup tradeoffs are retained in the [sanitized record](data/metadata-shutdown-0.8-results.json).

The Windows metadata process starts suspended, joins a kill-on-close Job Object and resumes hidden. Successful EOF receives up to 80 ms root grace; remaining job members are stopped/reaped, including descendants of an exited parent. Cancelled/error queries skip grace; cancellation during grace stays cancellation; task abort drops the job. Assignment failure does not run an unsupervised retry. Non-Windows/model-turn shutdown and native account/auth/permission policy remain unchanged.

Six real-process Windows tests cover EOF, stubborn descendants, exited parents, cleanup cancellation, abort/concurrent group isolation and actual catalog-query cancellation/abort. Local checks pass: 158 Rust tests (14 opt-in probes ignored), Clippy with denied warnings, debug build, production frontend build, 72 unit tests and 29 browser regressions, rustfmt/Prettier/docs. All seven composer, eight execution and seven account/quality/audit native offline scenarios pass with the 0.7.1 backend. Cross-platform CI, packaged installation and publication are separate release gates. [Full scope and reproduction](metadata-shutdown-0.8.md). No new live slot was spent.

This is the 0.7 acceptance record. Implementation, automated proof and environment-dependent live testing are distinct. The previous roadmap was retired prematurely; conditional performance proposals and acceptance work were not all completed. The [0.8 roadmap](roadmap-0.8.md) tracks their evaluation and targeted functional gaps without treating them as delivered features.

## Delivered implementation areas

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

**Conditional proposals not shipped:** full transcript virtualization, Rust Markdown/ANSI rendering, warm CLI pools and tuple-based metadata. Evaluation was incomplete: metadata received a synthetic comparison, but the other three did not receive alternative-prototype A/B comparisons. The evidence does not establish that these optimizations are unnecessary. Graph parser replacement also remains conditional. Functional limitations include static-only worktree previews and provider-specific child-session capabilities. The 0.8 roadmap retains these distinctions.

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

- A Windows native 1,000-message synthetic transcript loaded in about 0.5–0.6 s. Earlier synthetic jumps produced an isolated roughly 0.33–0.36 s worst frame interval; the later debug harness run on 10 November at 17:06 recorded about 563 ms load and a 782 ms maximum frame interval. These are individual synthetic samples, not a repeated release baseline or a universal smoothness claim. The later result was missing from the initial 0.7 documentation and warrants profiling before rejecting virtualization.
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

## 0.8 working-tree checkpoint — 11 November 2026

The owner interview is complete and the first composer tranche is implemented locally on the working tree based on `657c679`; the public version remains 0.7.0. Per-conversation text/attachments and immediate pending delivery now preserve source-chat ownership across delayed replies, rejected submissions and Compact. Drafts remain window-local and do not survive reload/restart.

The focused `node tests/native-composer.mjs` harness passed seven scenarios in Windows debug WebView2, using isolated data, real IPC/SQLite and offline Claude CLI fixtures. Controlled UI transport delays/failures cover single submission, late acceptance, draft recovery, accepted/rejected requests with unavailable first readback, new-chat creation and delayed Compact. A delivery check never automatically resends the prompt. The initial user insert rejects reused IDs, including collisions in another chat. Pending-state captures were inspected in both themes; uncertain-state recovery was inspected in light theme. An independent review found and verified fixes for two recovery issues; this is not a whole-app audit.

Applicable targeted checks passed: TypeScript, Rust debug build, three storage tests, thirteen Orchestra tests, two Playwright regressions (slash commands/sidebar filtering), Clippy with warnings denied, formatting and documentation references. Local reports/screenshots are under `.impeccable/review/native-composer*`. These checks are additional working-tree evidence, not a rerun or replacement of the 0.7 full-suite/CI record above. No paid/live inference, repeated release performance baseline, new package, version bump or publication occurred. See [0.8 roadmap](roadmap-0.8.md) for remaining work and the approved visual direction, which is not implemented by this tranche.

### Execution observability checkpoint

The next working-tree tranche adds a compact selected-turn execution disclosure, persistent bounded content-free provider snapshots, explicit tool lifecycle observations and a read-only status refresh. Native questions/permissions, provider silence, failure, interruption and normal return have distinct states. A provider return is not task-completion evidence; output-only child streams do not establish an active autonomous agent. Thirty seconds is a UI silence indicator, not a timeout or liveness proof. No silence-triggered request replay was added. Native fallback/resume behavior remains separate.

`node tests/native-execution.mjs` passed eight Windows debug/WebView2 scenarios using isolated IPC/SQLite and offline CLI fixtures: native question/answer, promise-only response with zero tool events and reload, tool start/end, provider failure, actual 30-second silence with read-only refresh and explicit Stop, delayed refresh after chat navigation, internal Orchestra interview snapshots retained after save/reload, and persisted interruption in Italian/dark theme. All seven composer scenarios passed again. Eight TypeScript state/reducer regressions and four Rust execution tests cover parallel selection, finite incomplete crash timings, activity-pressure bounds, privacy and durable scratch handoff; targeted protocol mapping tests also pass. TypeScript, debug build, Clippy with warnings denied, formatting and documentation checks passed. Local evidence is `.impeccable/review/native-execution-results.json` and `execution-*.png`.

Independent finish review: ship at the captured desktop scope after correcting dark-theme metadata contrast, disclosure affordance, scratch persistence, parallel selection, crash timing, activity pressure and refresh races. These are working-tree/offline results, not fresh live-account acceptance, full-suite CI, release performance or screen-reader certification. The historical live interruption cause remains unestablished. Backend pre-acceptance classification/catalog timing and cancellation are still incomplete; UI-only submission waiting is explicitly separate. See the roadmap for remaining quality, release-baseline and visual work.

### Quality, continuity and Auto checkpoint

The next tranche adds changed-only Lumen-owned notes/check evidence on native resume, current-chat evidence on account handoff, bounded follow-up route reuse, fresh-quota precedence and proportional review rules. Requested audits cannot silently disappear under quota/cooldown; review accepts only an entire unqualified approval, permits another primary account on the same provider and selects the actual reviewer's catalog model after fallback. Both and manual provider/model choices retain their contracts.

The fixed deterministic corpus improves from 2/14 to 14/14. The Windows native offline harness checks Claude/Claude/GPT continuity, evidence once, unrelated-chat isolation, quota precedence and independent audit/fallback selection. Independent React/TypeScript and Godot Mono C#/GDScript fixtures compile and detect intentional runtime defects; reference corrections pass. These controlled cases are not live model-intelligence proof.

The bounded direct-native GPT experiment yields complete requirements for the candidate on 2/2 valid React tasks and 3/3 Godot tasks, versus 0/2 and 1/3 for the preserved baseline. Thirteen slots were spent: six per GPT task plus one Claude OAuth failure; two React slots invalidated by a Windows short-path checker defect remain inside the six. The repaired checker passed external-TEMP offline gates before remaining calls. No quota/login retries, replacement pairs or extra slots were added. Claude acceptance and actual two-Claude topology remain unverified live. Input/output/cache and time are recorded, monetary charges unavailable; usage of the interrupted task is partial. No general speed, intelligence or official-app parity claim follows. [Full protocol/results](quality-experiments-0.8.md), [aggregate per-run record](data/quality-0.8-results.json).

Final working-tree checks pass: 151 Rust tests (13 opt-in probes ignored), 68 TypeScript tests, four targeted Playwright regressions, seven native quality scenarios, debug build, production frontend build, Clippy with warnings denied, rustfmt, Prettier and documentation checks. The final-build audit refinements are offline verified separately from the frozen live binary. Release/version, CI and non-Windows desktop acceptance are unchanged by this tranche.

### Windows performance and long-chat checkpoint

An isolated Windows x64 release reference now uses embedded production assets and synthetic 100/1,000/5,000-message short/mixed chats. The protocol fixes a 20% reduction gate on renderer task time for a 12-key draft-typing batch at 5,000 mixed messages. Baseline, excluded prototype, repeated baseline and final narrowed candidate each have 120 workload trials; focused A/B/A/B investigations add 160 scroll trials. No provider/account inference ran; the prior live budget remains at 13 spent slots.

The accepted full-transcript/composer isolation reduces the primary median from 734 to 271 ms (63.1%); the repeated baseline median is 1,582 ms. The same baseline's opening median varies from 1,386 to 2,921 ms, so the absolute baseline is a nonstationary local reference, not a stable startup/hardware-wide target. The first per-message-memo prototype is excluded. The final matched scroll p95 is 37.5/37.6 and 33.4/33.3 ms with visible/focused windows, equal viewport/DPI and identical transcript HTML. All content remains mounted; CSS and MessageView match the entry snapshots. No general opening, streaming or memory saving is claimed.

Eight native release scenarios pass on the exact candidate hash: historical find/selection/code, copy/disclosure/DOM retention, link keyboard access and local image focus, latest-body streaming, Compact without generation, current-chat callbacks and Both lane grouping. The seven composer and eight execution debug/offline scenarios also pass with the final frontend. Light/dark captures were inspected. Final type/production build, 68 TypeScript tests, all 29 Playwright tests, Prettier and documentation checks pass. These remain distinct from native release evidence; no new backend change or paid-call acceptance is implied. [Full protocol/remaining scope](performance-0.8.md), [aggregate data and excluded prototype](data/performance-0.8-results.json).

### Initial-opening and virtualization checkpoint

The next fixed comparison records 360 Windows release openings: 20 per 100/1,000/5,000 short/mixed workload and baseline/plain/virtual arm in one isolated instrumented binary. The accepted literal-paragraph path reduces the 5,000-message mixed median from 1,259.7 to 932.4 ms (26.0%; paired median 25.8%). Complete 5,000-message HTML hashes match; all history remains mounted. Native find/full selection and targeted first/last paragraph AX projections match on 1,000 mixed messages. No screen-reader certification or full 5,000-message selection/AX export is claimed. Two exploratory diagnostics were stopped and retained separately; the fixed timing series is complete.

The variable-height window prototype fails historical find/selection/AX content gates and is excluded. Ordinary builds remove the experiment module and ignore its query selector. The 80-sample secondary comparison does not reproduce the initial small-chat p95 and 1,000-message typing signals. A browser regression exposed a slow URL predicate on huge literal strings; final code uses linear delimiter/whitespace handling, with independent parser-oracle coverage for 680,000 characters and a long internal-space run. No provider inference or budget increase occurred; the ledger remains 13 spent slots. [Protocol and scope](initial-open-0.8.md), [aggregate record](data/initial-open-0.8-results.json).

The corrected ordinary release passes 20 matched launch pairs independently: 1,405.9 → 1,046.2 ms opening (25.6% reduction), 124.7/125.9 ms typing (+1.0%). The first final-binary confirmation mixed smooth/instant scrolling through unequal navigation after media emulation; all 40 observations remain excluded and documented, with direct reproduction of the cause. Corrected preparation is equal for both arms. Normal-build content/query/AX gates and eight native release scenarios pass on exact SHA-256 `f59bdb4688683ea21e31b8254fc830e5a71bf137d8beba292600ccf0e11db92b`. Type/production release build, 72 unit tests, 29 browser regressions, seven composer/eight execution debug scenarios, formatting and documentation checks pass; captures in both themes were inspected. Version/publication and non-Windows/screen-reader acceptance remain unchanged.

### Turn waiting and CLI startup checkpoint

Content-free monotonic timing offsets now augment existing execution snapshots; accepted direct submissions persist preparation duration. Transport boundaries are captured at their source before callback queues; normalized first event/text remains controller-observed. Unknown labels/future timestamps are rejected; timing signals use existing snapshot persistence without extra per-marker IPC/database writes. Account, auth, routing, permissions and native resume behavior are unchanged.

The fixed Windows release collection completes 80 offline-fixture turns, 20 initial/20 continuation per provider, on SHA-256 `4a9e8481d23eb16cbc0353575b45b8262fb19b829467cde2e5fd290112082244`. It verifies preparation/timing persistence, protocol/request markers, native session continuity and absence of raw timing control activity. Warm preparation medians are 1 ms and acceptance approximately 9 ms; first account uses cost 734/686 ms acceptance. These reflect existing cache/fixture behavior, not measured live model speed or a new optimization.

Separately, actual installed Claude Code 2.1.314 and Codex CLI 0.167.0-alpha.4.1 complete 20 metadata/version samples each in isolated configuration, without copied credentials, login or inference. Median process-to-initialize-ack is 531/186 ms; complete metadata calls are 953/588 ms, including shutdown medians of 405/387 ms. The Rust probe driver is debug; the installed CLIs and native offline release records remain separate. No cold OS, actual provider TTFT, live account topology, Auto inference or Orchestra worker-scheduling acceptance is inferred. The metadata shutdown path is a future optimization candidate; no process pool is added.

152 ordinary Rust tests pass, with 14 opt-in probes ignored; the one explicitly selected metadata probe also passes without inference. Four debug smoke turns, eight execution scenarios and seven quality/account/audit scenarios pass with the instrumented backend, including question/error/silence/cancel and stored internal Orchestra interview evidence. Clippy denies warnings; debug/release/frontend builds, rustfmt, Prettier and documentation checks pass. [Protocol and limits](turn-latency-0.8.md), [sanitized data](data/turn-latency-0.8-results.json). The live ledger remains 13 spent slots; no new live pair, version bump, publication or desktop/platform certification occurred.
