# Initial chat opening and virtualization — 0.8 experiment

Protocol fixed before prototypes on 11 November 2026. Preserve the existing working tree and the accepted transcript/composer isolation. No version bump, publication or new live/model calls; the previous live budget stays at 13 spent slots.

## Fixed protocol

- Windows x64 release, embedded production assets, a fresh isolated identifier/product name/database/profile; no installed-app accounts or conversations. Same renderer/build and synthetic fixture data for experimental arms; normal build checked separately before integration.
- Baseline: current complete Transcript/MessageBody. Candidate: a conservative single-paragraph plain-text path, with the existing Markdown implementation as semantic oracle and fallback. Experimental virtualization: variable-height window with measured rows and overscan; offscreen rows unmounted. It is an experiment, not an accepted product behavior.
- Workloads: 100/1,000/5,000 short/mixed messages from the existing deterministic fixture generator, 20 openings per workload/arm, alternating arm order. No discarded slow runs; record process/renderer state, content counts, frame/long-task and memory evidence separately.
- **Primary:** opening a 5,000-message mixed chat to the latest fully rendered response and two animation frames; require at least 20% lower median on paired samples. **Hard gates:** complete searchable/selectable semantic content and preserved actions. A virtual tail with missing history never counts as a fully ready transcript, however fast it paints.
- Secondary: full-document readiness, IPC time versus UI task/script/layout work, long tasks, DOM/heap/private process bytes, smaller-chat opening, typing and scroll. Investigate >5% reproducible regressions. First-process/cold OS, provider latency and non-Windows GUI are not inferred.
- Independent acceptance: exact HTML on eligible text versus the installed ReactMarkdown/GFM parser; adversarial Markdown/HTML/entities/links/Unicode/whitespace. Native find and selection across first/last messages; accessibility role/text exposure and keyboard actions; images, code, links, disclosures, streaming, Both, Compact and current-chat callbacks. Real screen-reader certification must remain distinct from accessibility-tree evidence.
- Integrate only candidates that pass content/accessibility gates and measured benefit. Preserve a failing virtualization prototype and its measurements rather than hiding its missing content behind a speed claim. No new product setting, fallback button or visual redesign is pre-authorized by this experiment.

## Results and decision — 11 November 2026

The complete fixed series contains **360 openings**, 20 per workload/arm, alternating order in one instrumented release executable. All measured windows are visible/focused, at 827×547 CSS pixels and device scale 1.5, WebView2 157.0.4341.46. No timing samples were discarded. [Aggregate record](data/initial-open-0.8-results.json).

| Messages | Content | Baseline median | Accepted paragraph path | Reduction |
| --- | --- | --- | --- | --- |
| 100 | Short | 31.9 ms | 24.6 ms | 22.9% |
| 100 | Mixed | 58.9 ms | 52.3 ms | 11.3% |
| 1,000 | Short | 105.5 ms | 59.8 ms | 43.3% |
| 1,000 | Mixed | 269.6 ms | 203.9 ms | 24.3% |
| 5,000 | Short | 440.4 ms | 156.5 ms | 64.5% |
| 5,000 | Mixed — primary | **1,259.7 ms** | **932.4 ms** | **26.0%** |

The median paired reduction on the primary workload is 25.8%, independently exceeding the fixed 20% threshold. Its p95 opening decreases from 1,301.5 to 1,030.1 ms. These are warm chat openings, not cold app startup or model/provider response times. Host/GC history still limits absolute timing and memory conclusions.

The corrected **ordinary release** independently passes 20 matched launch pairs: **1,405.9 → 1,046.2 ms (−25.6%; paired median −25.9%)**. Typing task medians are 124.7/125.9 ms (+1.0%), within the secondary threshold. Its SHA-256 is `f59bdb4688683ea21e31b8254fc830e5a71bf137d8beba292600ccf0e11db92b`; normal-build content parity and eight native release scenarios pass on that exact executable. The final source has 72 unit tests, all 29 Playwright regressions, production type/build, Prettier and documentation checks passing. Seven composer/eight execution debug/offline scenarios also pass in this tranche. Native light/dark captures were inspected; no new backend/release/platform acceptance is inferred.

`plainParagraph` recognizes only an unambiguous single literal paragraph. It renders escaped React text with the same paragraph element and whitespace as the installed parser; Markdown, GFM, URLs, entities, escapes, multiline text and ambiguous syntax keep the original ReactMarkdown path. It does not truncate text or replace link/image handlers. Four independent-oracle tests include 1,200 fixed generated strings, targeted syntax/Unicode cases and a 680,000-character literal message. The previous complete Transcript/composer isolation remains intact.

The **variable-height virtualization prototype is excluded**. It measures mounted rows and uses overscan/spacers, but unmounts historical content. Native find cannot reach the first message; cross-history selection loses it; targeted accessibility evidence loses its text. Its small tail-paint timing is deliberately not presented as full-transcript readiness. It stays under test fixtures and an explicit build-time experiment flag; ordinary release builds remove its import/chunk and ignore experiment query parameters. No user setting or visual redesign is added.

## Preservation evidence and limits

- Baseline and accepted path have identical full transcript HTML hashes for all 5,000 mixed messages. Every message remains mounted, with identical DOM counts and existing layout/scroll policy.
- Native find and complete selection on the 1,000-message mixed fixture retain both endpoints; selected-text hashes match. Targeted first/last paragraph accessibility subtrees have identical role/name/ignored projections. These are native WebView2 observations, **not full screen-reader certification**.
- Two preliminary 18-sample pilots were stopped during expensive 5,000-message full accessibility-tree/selection diagnostics. They remain in the ignored evidence directory and are excluded from the fixed timing series. The final protocol records the bounded 1,000-message native probes separately from complete 5,000-message HTML equivalence. No 5,000-message full selection or screen-reader pass is inferred.
- The main series shows a noisy 100-message mixed opening p95 and approximately 5% higher 1,000-message mixed typing median. A separate **80-sample** baseline/plain investigation does not reproduce either: small-chat opening p95 is 71.2/68.1 ms; 1,000-message typing medians are 34.6/34.1 ms. A roughly 1 ms typing increase on 100 messages in this secondary series is absent from the main series. These observations remain visible; no general typing benefit is claimed here.
- The first full browser regression run exposed two timeouts on huge single-message context fixtures. An unanchored URL recognizer could repeatedly scan a long literal word. It was replaced with a linear literal delimiter check and an explicit long-message oracle case. The two affected browser regressions then pass. The original experimental binary remains frozen; a separate paired confirmation is required for the corrected ordinary-build candidate, with both hashes recorded.
- The first 40-sample final-binary confirmation used unequal navigation after media emulation. The ordinary candidate initialized the installed reduced-motion hook before emulation, keeping smooth scrolling; the reloaded baseline initialized it afterward, using instant scrolling. This produced an apparent typing regression. `initial-open-motion.mjs` directly reproduces smooth/instant calls while the media query is true in both cases. That entire confirmation is retained separately and excluded from matched acceptance; a four-sample setup probe precedes the corrected 20-pair comparison. Both arms now navigate equally after media setup. The original three-arm series already did so and remains unchanged.
- Heap and process-private memory are separate measurements. Private process and read-only IPC probes occur once per workload/arm; they do not establish memory or IPC improvements. No new scroll-speed claim is made.

## Reproduction

Run native experiments serially with Node 24, installed Rust/Tauri/WebView2 and existing project dependencies. Use a fresh root to start a new experiment; do not overwrite the preserved binary or budget ledger. The overlay changes identity/product name/profile, so its executable is a benchmark artifact, not a normal distributable release.

1. Set `LUMEN_PERF_ROOT=.impeccable/initial-open` and run `node tests/performance-prepare.mjs` before edits. Source excerpts and a separate identifier/profile are retained.
2. Set `VITE_TRANSCRIPT_EXPERIMENT=1`; build `npx tauri build --no-bundle --config .impeccable/initial-open/config.json`; freeze `src-tauri/target/release/lumen.exe` under the root's `experiment/lumen.exe`. One executable selects baseline/plain/virtual internally, using identical fixture data.
3. Run `node tests/initial-open-native.mjs`, default 20 repetitions. The result contains all 360 samples, content gates and captures. `LUMEN_OPEN_SECONDARY=1` runs only the separate 100/1,000 mixed baseline/plain investigation and writes `secondary-results.json`, preserving the main record.
4. Unset `VITE_TRANSCRIPT_EXPERIMENT`, build again with the same overlay and freeze under `candidate/lumen.exe`. `node tests/initial-open-stock.mjs` checks ordinary-build HTML/selection/AX parity and verifies query parameters cannot enable virtualization. It also checks that the current normal frontend assets contain no prototype chunk or runtime selector.
5. Run `tests/performance-acceptance.mjs` with this root and `LUMEN_PERF_EXE` pointing at the frozen candidate. Run the composer/execution offline harnesses serially and applicable frontend checks.
6. `node tests/initial-open-confirmation.mjs` runs 20 alternating launch pairs on the primary workload against the frozen experimental baseline. Each launch has an excluded 100-message warmup; this verifies a changed final ordinary binary without overwriting the original comparison.
7. `node tests/initial-open-report.mjs` emits sanitized aggregates under `docs/data/`; raw traces, isolated DB/profile, source/binary snapshots and images remain local in `.impeccable/initial-open/`.

No live inference was used. The existing live ledger remains at **13 spent slots**. Full virtualized history with preserved native find/selection/accessibility, cold startup, provider/CLI attribution, larger mixed-media and non-Windows/screen-reader acceptance remain separate open work.
