# Local assistant — 0.7

The optional local helper supplies chat titles, editable commit-message drafts and handoff summaries. It never commits, sends a prompt to a remote inference provider or overrides a manual draft. Service/model/timeout failures preserve the normal manual flow.

## Integration

- `localAssistant { enabled, model, autoTitles, timeoutMs }` is persisted in app settings and rendered through `LocalAssistantSettings`. It is off by default. `ollamaModel`, used for the conversational provider's preferred model, is separate.
- `ollamaTransport()` invokes native `ollama_models` / `ollama_generate`. The Rust host uses the official loopback Ollama API; the webview does not contact the daemon directly. No automatic model pull, cloud alias or non-loopback inference is allowed.
- `instance.ts` holds the configured helper. `autoTitle.ts` changes a derived title only after the first response and rechecks user ownership; a manually renamed chat is preserved.
- `DraftPanel` integrates commit-message and edited handoff drafts in the application. Copy does not commit; Compact uses the edited summary with durable task state.
- `httpTransport` remains a controlled transport/test implementation. The desktop app uses native IPC.

The separate `ollama_agent.rs` conversation runner executes Moli function calls/JSON actions and returns their results to the local model. It does not depend on this helper being enabled.

## Verification

Frontend tests cover triggers, title ownership, draft editing, failures/cancellation and loopback checks. Rust tests cover local catalogs, cloud exclusion, missing models and tool loops. Opt-in live probes require a running local Ollama with an already installed model; they are not ordinary CI work.

[Browser integration](../../../docs/moli.md) · [Validation](../../../docs/validation.md)
