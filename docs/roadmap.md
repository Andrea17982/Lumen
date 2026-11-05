# MVP → v1 → v2

## MVP / 0.2 alpha — this repository

| Original requirement | Status |
| --- | --- |
| Tauri 2 shell, Rust core, React/TypeScript, Tailwind, Framer Motion | Implemented |
| Sidebar, title, rename, state, pin, search | Implemented; title is a local first-message heuristic |
| No artificial saved-session count limit | Implemented through persistent/paginated storage; not infinite system resources |
| Streaming text | Claude partial stream; Codex completed-message JSONL updates |
| Thinking/tools/edits collapsible | Implemented for exposed provider events; no private reasoning access |
| Auto / Claude / GPT, optional account/model/effort | Implemented |
| Isolated multi-account directories and official login | Implemented; multi-identity OAuth acceptance still required |
| Automatic provider/model/effort | Deterministic initial rules; Codex default model delegated to CLI |
| Automatic sub-agents | Bounded independent worker calls; no recursive/native subagent scheduler |
| Writer/reviewer and pair work | Writer/reviewer and parallel perspectives with synthesis implemented |
| Light/dark, glass UI, English interface | Implemented |
| Instant start / very low memory | Design goals; no benchmark claim |
| Windows | Build/release target |
| Linux | Secondary source-build target; manual login and no real-device validation |

Alpha gates: automated frontend/Rust checks, UI behavior/render review, Windows installer build, then user-driven live-provider acceptance. Do not equate a successful compiler run with validated paid inference or multi-account separation.

## v1 — reliability and deeper integration

- Acceptance-test two distinct accounts per provider, token refresh, login cancellation, expired login and rate limits without credential leakage.
- Use provider-native thread resume and context compaction, keeping context transitions visible.
- Add app-server / supported event transports for richer Codex token streaming and permission approval UX.
- Evaluate task-aware fast model selection using the official catalogs implemented in 0.2. Catalog discovery is not proof of per-request entitlement.
- Paginate messages and virtualize long transcripts; remove unused workspace-lock entries.
- Versioned database migrations, export/import, encrypted-at-rest option and recovery tooling.
- Code-signed Windows installer, startup/memory measurements on representative hardware, Linux CI and desktop acceptance.
- Component-level frontend decomposition, IPC contract generation and simulated-process lifecycle tests.

## v2 — evaluated orchestration

- Evaluated classifier-assisted planning, with deterministic fallback and inspectable decisions.
- Explicit budgets, quota-aware scheduling within provider terms, task-aware model selection and account health checks.
- Worktree-isolated workers, bounded recursive delegation, merge/review stages and reproducible activity artifacts.
- Optional attachments, richer diffs, terminal/permission handling and per-session policies.

The roadmap is intent, not a delivery promise. Community feedback and measured behavior should determine priority.
