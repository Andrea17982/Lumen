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

## Checkpoint restore safety

Source: [CheckpointRestoreSafety.ts](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/orchestration-v2/CheckpointRestoreSafety.ts), `isCheckpointRestoreIsolated` and its `contains` helper.

Lumen independently ports the idea to Rust in `src-tauri/crates/lumen-core/src/autonomy.rs` (`overlaps`, `rewind_blocker`) and `turns.rs` (`rewind_scope`, `checkout_conflicts`). A restore is treated as rewriting the whole checkout, and another conversation counts as sharing it when either folder contains the other. Paths are canonicalized, and a path that no longer exists is compared as given. No TypeScript or Effect code is imported.

Lumen-specific changes: the restore scope is the Git top level rather than a thread worktree. Lumen checks both conversations that are running and conversations that changed since the chosen message. It does not exempt idle side chats, since Lumen has no read-only side-chat class. It refuses when a discarded native CLI turn ran outside the restored checkout, which covers the case where the provider conversation cannot be rewound consistently. Native resume markers are already cleared on truncation. Lumen also adds a pre-restore snapshot (an idea from Hermes Agent's rollback, not code) and byte-exact handling of attributed and filtered files. These are covered by Lumen tests, not by the upstream suite.
