# Native approvals and lifecycle — 0.7

This crate owns immutable native approval requests and decision delivery; `GatekeeperPanel.tsx` renders their review cards. The host wires Claude through `native_claude.rs`/`approvals.rs` and Codex through `approvals_codex.rs`. Codex app-server is also used outside Ask-first mode; native questions are handled separately by `interactions.rs`.

## Ownership contract

A scope binds the Lumen chat, provider, native session/thread/turn and process connection. Requests keep actual native identities and the original input. UI decisions carry request/scope/action, not a rewritten command, provider input or arbitrary JSON-RPC reply.

Approve/reject/cancel decisions are checked against the current request and delivered once. Preview changes, disconnection, cancellation, timeout and recovery retire stale requests. `delivery_is_current` is checked immediately before a write, and `complete_delivery` records actual delivery. Never reconnect and replay an earlier acceptance.

Unsupported callbacks/previews remain unavailable or declined. A parse error must not fall back to a less restricted runner. Terminal text, model questions and earlier chat approvals are not new native permission grants. Exact command rules do not become broad shell permissions.

## Codex

The adapter resolves the account's actual binary, gathers supported schema/initialize evidence and starts app-server. Thread/turn requests assert the real cwd and current native sandbox/approval policy, including resume. The reader continues while approval futures wait; one writer queue serializes responses.

Observed items/patches and `serverRequest/resolved`/turn completion update or retire requests. Supported command/file callbacks need complete representable previews. Network/additional permissions, persistent grants and unknown protocol shapes need dedicated adapters; they must not be relabeled as shell approval.

## Claude

The main executable branches before Tauri for `--lumen-gatekeeper-mcp`; helper stdout contains protocol frames only. The native bidirectional stream initializes before model input, keeps stdin open and handles questions/control callbacks. The session's loopback MCP bridge remains available for supported permission/capture tools.

Each real connection receives a fresh endpoint/capability and scope. Tokens/credentials never belong in transcript logs. Input previews describe the proposed operation, not an attestation of the current disk. The session directory is not proof of a Bash tool's effective cwd after earlier commands.

Bridge loss or cancellation retires requests and interrupts the native turn. Disconnecting the broker is not proof that every OS process exited; the host owns process-tree cleanup/recovery separately.

## Verification

Controlled crate/native tests cover concurrent callbacks, approve/reject/cancel, timeouts, invalid/stale previews and recovery. Windows native fixture tests exercise the panel and question flows. Earlier real Claude/Codex approved-write probes are historical observations; latest Claude live inference remains deferred by the owner. Clean Windows installed-helper and cross-platform build checks are part of the 0.7 pipeline. See the canonical [validation record](../../../docs/validation.md) for exact coverage.

The reduced schema fixtures are synthetic/generated protocol excerpts without personal account data. Regenerate evidence from the actual installed CLI when changing an adapter. Preserve official authentication and the applicable native permission policy.
