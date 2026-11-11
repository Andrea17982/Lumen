# Windows performance and long chats — 0.8 working tree

Protocol fixed on 11 November 2026 before UI prototypes. Preserve the current working tree based on `657c679`, including the first three tranches. No version bump, installer, publication or paid inference is part of this experiment.

This record belongs to the fourth tranche, measuring draft-typing work. The subsequent [initial-opening experiment](initial-open-0.8.md) measures the conservative paragraph path and excludes a windowed virtualization prototype; it does not rewrite the timings or acceptance scope below.

## Workload and decision gate

- Native Windows x64 release executable, bundled production frontend, isolated application identifier/database/WebView2 profile. No original account or conversation is opened. Debug results remain separate.
- Synthetic 100 / 1,000 / 5,000-message chats, short and mixed long Markdown/code/table/activity/child output. Same data and viewport for baseline/candidate; 20 repetitions per workload, warm OS/profile distinguished from first-process launch. No cold-OS claim.
- Primary metric chosen before prototypes: UI-thread task time for a fixed 12-key draft-typing batch on the 5,000-message mixed chat. Require at least 20% lower median, repeatable beyond sample variability. Input-to-next-paint timing remains a separate user-facing measure.
- Secondary: chat-open to two animation frames after complete transcript mount, input latency, streaming work, continuous scroll and large jumps, frame intervals/long tasks, DOM and JS heap, process-tree private bytes/process count. Private bytes are not summed shared working sets and not provider memory. Investigate >5% regressions; correctness gates take precedence.
- Keep all text, native find/selection/copy, historical links/images, disclosures, keyboard navigation, permissions, account routing and Compact reachable. Do not silently truncate history or label a partial window as full virtualization acceptance. Compare/rerun against a frozen baseline executable and sources; no user-facing benchmark controls.
- Start with measured render invalidation and existing content-visibility. Evaluate virtual rendering separately if needed, recording functionality gaps and keeping inconclusive/deferred work visible. Do not trade content accessibility for a benchmark win.
- Local measurements have no inference quota cost. The previous live budget ledger remains unchanged; no additional paid runs are needed for a render-only change.

## Measured environment and baseline

Windows x64 build `10.0.26300`, Intel i9-13980HX, 32 logical processors, WebView2 `157.0.4341.46`. Builds use the existing release profile (size optimization, LTO, one codegen unit) and embedded production assets. The isolated window is 1240×820 physical pixels; the focused scroll checks record an 827×547 CSS viewport at 1.5 device-pixel ratio. Timing uses light theme and reduced motion; light/dark captures are visual checks, not a complete dark-theme timing matrix.

The [aggregate measurements](data/performance-0.8-results.json) preserve hashes, environment, all workloads, p95 and controls. Baseline hash: `93a6cc77c9cd3aed51b5d118ea582b4289b59ac9397172e453ee78fe55bc7e6a`. Accepted candidate: `2ace96b61ed141c6edc8ec1077328d9c86e9bd799cefd6dfb42327261774dc3a`. The baseline is the working tree after the first three tranches, not the published 0.7 installer.

Collected **480 full workload trials**: 120 baseline, 120 first prototype, 120 repeated baseline and 120 narrowed final candidate. Each series has 20 trials for each size/content combination. Two focused A/B/A/B investigations add 160 scroll trials. A six-case pilot and infrastructure probes are separate. All are local synthetic runs; the existing live ledger remains at 13 spent slots, with no new inference.

Absolute timing is **nonstationary** on this host. The identical baseline binary's 5,000-message mixed opening median changes from 1,386 ms to 2,921 ms in the repeated control. Its typing-task median changes from 734 ms to 1,582 ms. This is observed variability, not an established diagnosis of its cause. These datasets are a first reproducible local release reference, not a stable cold-start, hardware-wide or provider-latency baseline. No slow samples were silently removed.

## Accepted improvement: isolate draft edits from the transcript

App previously rebuilt every MessageView, reparsed reasoning/activity and recalculated history projections when the composer changed. `Transcript` now owns grouping/rendering behind a memo boundary; message/submission projections, token estimates and usage lookup reuse unchanged inputs. The execution summary also skips unrelated parent updates, while retaining its own active clock. Stable action callbacks forward to the latest committed chat/permissions; a late merge readback cannot overwrite another selected chat.

The final candidate keeps **ordinary MessageView rendering on actual transcript updates**. It preserves the complete DOM, existing Markdown/image/link behavior, Both lanes, content-visibility, styles and scroll policy. It introduces no new UI control, prompt, permission gate, provider call or virtualization mode. MessageView and CSS match the entry snapshots byte for byte.

| Typing workload | First baseline task ms | Final candidate task ms | Repeated baseline task ms |
| --- | --- | --- | --- |
| 100 short | 23.7 | 34.6 | 52.2 |
| 100 mixed | 26.1 | 36.8 | 60.8 |
| 1,000 short | 120.4 | 56.7 | 275.3 |
| 1,000 mixed | 144.2 | 70.7 | 337.6 |
| 5,000 short | 600.4 | 213.0 | 1,333.7 |
| **5,000 mixed, primary** | **734.1** | **271.2** | **1,582.4** |

The prespecified primary median improves **63.1% against the first baseline and 82.9% against the repeated baseline**, exceeding 20% in both comparisons. This measures renderer task time for the fixed 12-key batch, not whole-app speed, model speed, INP or billed token savings. Small-chat results and all secondary measures remain visible; the first-to-last comparisons cannot establish a general opening, streaming or memory improvement under the observed drift.

## Excluded prototype and secondary investigation

The first prototype also memoized each MessageView. It passed the primary metric but showed unexplained differences in focused scrolling despite identical HTML. It is **not integrated**; its hash, primary figures and scroll observations remain in the aggregate record. Narrowing removed that extra change rather than declaring the primary win sufficient.

The final focused 5,000-message mixed A/B/A/B check uses 20 scroll trials per launch and records visible/focused windows, equal viewport/DPI and identical full transcript HTML hashes. Matched p95 frame intervals are **37.5 / 37.6 ms** and **33.4 / 33.3 ms** (baseline/candidate), within approximately 0.3%; medians are also matched. This resolves the observed regression at the tested scope. It does not certify universal smoothness: large jumps and initial layout can still stall.

Both builds mount all 5,000 articles, around 100,239 total DOM nodes on the mixed fixture. Full virtualization and initial-mount scaling remain open candidates. They were not prototyped or accepted in this tranche; preserving find, selection, keyboard and accessible history remains a gate for that work. No conclusion that virtualization or parsing work is unnecessary follows from the draft-typing result.

Process-tree private bytes and JS heap are recorded separately. Private bytes have only two observations per workload/arm and reflect an interleaved warm-process history, not standalone per-chat memory. Ordinary GC and earlier larger chats affect these values. No 15% memory-saving acceptance or provider-memory claim is made. Frame/key proxies are named explicitly; they are not presentation-feedback INP measurements.

## Verification and remaining scope

Eight native **release** scenarios pass on the exact final binary: historical browser find, cross-history selection/code, response copy with disclosure/DOM retention, keyboard links and local-image enlargement/focus, latest-body streaming, Compact handoff without generation, latest-chat callbacks and adjacent Both lanes. The harness checks native responsive rendering at 1240/800/720 CSS widths. Light/dark captures preserve the incumbent composition. External-editor launch, full write-worktree integration and screen-reader certification are not inferred from these cases.

The seven native composer and eight native execution-observability **debug/offline** scenarios also pass with the current frontend, including delayed submission/Compact, questions, silence, refresh races and Italian/dark reload. Final frontend checks pass: typecheck/production build, 68 unit tests, all 29 Playwright tests, Prettier and documentation references. These remain separate from the native release results. No backend Rust, account authentication, routing, permission policy, version or release workflow is changed.

Remaining work includes stable absolute baseline/first-run startup samples, provider/CLI handshake attribution, broader mixed-media timing, full virtualization with content-access gates, screen-reader acceptance and non-Windows desktop checks. Initial-mount, scrolling and memory are not declared generally optimized. Release packaging/CI and publication remain separate.

## Reproduction

Use Node with `node:sqlite` (the recorded environment uses Node 24), installed Rust/Tauri/WebView2 and existing dependencies. No model/account is needed.

1. Before candidate edits, run `node tests/performance-prepare.mjs`; it creates an isolated identifier/config and source excerpts, or reuses existing evidence without resetting it.
2. Build `npx tauri build --no-bundle --config .impeccable/performance/config.json`. Freeze `src-tauri/target/release/lumen.exe` as `.impeccable/performance/baseline/lumen.exe` before edits.
3. Run `node tests/performance-native.mjs baseline` (20 repetitions by default). Implement/build the candidate with the same overlay and freeze it under `candidate/lumen.exe`.
4. Run `node tests/performance-native.mjs candidate`, then `node tests/performance-native.mjs confirmation` for the frozen baseline control. Native harnesses run serially.
5. Run `node tests/performance-scroll.mjs` for focused A/B/A/B and `node tests/performance-acceptance.mjs` for correctness on the exact candidate hash. The acceptance harness removes only its generated sessions/files afterward.
6. `node tests/performance-report.mjs` emits sanitized aggregates; raw traces, fixture DB/profile, captures and executable/source snapshots stay ignored under `.impeccable/performance/`. The historical excluded-prototype archives belong to this experiment; they are not application assets.

The overlay deliberately changes application identity/profile; its executable is a benchmark artifact, **not a distributable normal-identity build**. Build without the overlay for normal application/release work. Do not use the installed app's profile or replace the frozen executable during a measurement series.
