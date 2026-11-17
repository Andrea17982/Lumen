# T3 Code adaptations in Lumen

Upstream: [T3 Code](https://github.com/pingdotgg/t3code), created by Theo / pingdotgg and contributors; copyright holder **T3 Tools Inc.**

Source revision: `375343aaf35133f659435554cd01634bab333a7d` (MIT; the unchanged upstream text is in `LICENSE.txt`).

## Worker approval inbox

Source: [ChildThreadApprovals.tsx](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/web/src/components/chat/ChildThreadApprovals.tsx#L19-L80), the behaviour of naming the child thread, showing the reason and disabling replies once the child process is gone.

Lumen reimplements that behaviour without copying upstream code: a `Worker` identity (agent, subtask id, subtask title) on Gatekeeper sessions and approval requests (`src/features/gatekeeper`), worker-tagged native questions (`orchestra.rs`, `NativeQuestions.tsx`), and an expired card when the worker's connection is disconnected. Lumen-specific changes: the existing Gatekeeper journal, scope checks and one-shot decisions are reused unchanged; parallel workers of one chat keep separate connections; journals written before this change load without a worker.

## Command receipts

Sources:

- [OrchestrationCommandReceipts.ts](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/persistence/OrchestrationCommandReceipts.ts), particularly `insertIfAbsent` and `getByCommandId`.
- [Orchestrator.ts](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/orchestration-v2/Orchestrator.ts), `dispatchWithReceiptEffect` and receipt replay.

Lumen independently ports the insert-if-absent / lookup / replay design to Rust and rusqlite in `src-tauri/crates/lumen-core/src/command_receipts.rs`, `store.rs` and schema migration 5. IPC request IDs and transport-only recovery are integrated in `lumen-ipc` and `lumen-daemon`. No TypeScript/Effect runtime or upstream application bundle is imported.

Lumen-specific changes: UUID validation; binding IDs to command/argument SHA-256; per-ID synchronization without serializing unrelated commands; a limited command allowlist; no raw request arguments or API keys in the receipt table; persisted successful/error replies; one transport retry using the same ID. A pending claim after cancellation/crash is never automatically executed again. This is deliberately not T3's transactional event/outbox architecture and does not promise exactly-once external tool effects. Receipts have no automatic expiry, so old IDs cannot silently regain execution authority.

Tests cover concurrent duplicates, lost replies, argument mismatch, invalid/missing IDs, saved errors, database reopen, schema-4 data preservation, uncertain claims, and client retry identity. These are Lumen tests, not a claim that the upstream test suite ran here.
