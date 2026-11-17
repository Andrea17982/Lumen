# T3 Code adaptations in Lumen

Upstream: [T3 Code](https://github.com/pingdotgg/t3code), created by Theo / pingdotgg and contributors; copyright holder **T3 Tools Inc.**

Source revision: `375343aaf35133f659435554cd01634bab333a7d` (MIT; the unchanged upstream text is in `LICENSE.txt`).

## Command receipts

Sources:

- [OrchestrationCommandReceipts.ts](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/persistence/OrchestrationCommandReceipts.ts), particularly `insertIfAbsent` and `getByCommandId`.
- [Orchestrator.ts](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/orchestration-v2/Orchestrator.ts), `dispatchWithReceiptEffect` and receipt replay.

Lumen independently ports the insert-if-absent / lookup / replay design to Rust and rusqlite in `src-tauri/crates/lumen-core/src/command_receipts.rs`, `store.rs` and schema migration 5. IPC request IDs and transport-only recovery are integrated in `lumen-ipc` and `lumen-daemon`. No TypeScript/Effect runtime or upstream application bundle is imported.

Lumen-specific changes: UUID validation; binding IDs to command/argument SHA-256; per-ID synchronization without serializing unrelated commands; a limited command allowlist; no raw request arguments or API keys in the receipt table; persisted successful/error replies; one transport retry using the same ID. A pending claim after cancellation/crash is never automatically executed again. This is deliberately not T3's transactional event/outbox architecture and does not promise exactly-once external tool effects. Receipts have no automatic expiry, so old IDs cannot silently regain execution authority.

Tests cover concurrent duplicates, lost replies, argument mismatch, invalid/missing IDs, saved errors, database reopen, schema-4 data preservation, uncertain claims, and client retry identity. These are Lumen tests, not a claim that the upstream test suite ran here.

## Bounded wire projections

Source: [WireProjection.ts](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/orchestration-v2/WireProjection.ts) (idea: project large payloads to a bounded head with totals and a truncation marker, and fetch detail on demand).

Lumen independently implements the idea in Rust in `src-tauri/crates/lumen-core/src/wire.rs`, applied to `get_messages` snapshots and to live `lumen:run` events after they are persisted in full (`host.rs`). Detail is fetched with `activity_detail` and located by a SHA-256 digest of the full text, so no extra table or identifier is stored. Lumen-specific: a fixed 16 KiB limit on UTF-8 boundaries, only plain-text kinds (`tool`, `command`, `file`, `tool-end`), and a UI control in `MessageView.tsx`. No upstream code is copied.

## Transcript regression scenarios

Source: [MessagesTimeline.browser.test.tsx](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/web/src/components/chat/MessagesTimeline.browser.test.tsx) (scenario list: long streaming, variable-height rows, expand/collapse, ordering, keyboard and search).

`tests/transcript.spec.ts` re-creates those scenarios as offline Playwright tests against Lumen's browser preview with fixture data. Tests and assertions are written for Lumen; no upstream test code is copied.
