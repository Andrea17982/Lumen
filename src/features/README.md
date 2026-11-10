# Integrated feature modules — 0.7

The Rust host is the execution authority; frontend modules render state and collect scoped decisions.

| Feature                   | Host / renderer integration                                                                     |
| ------------------------- | ----------------------------------------------------------------------------------------------- |
| Orchestra scheduling      | `src-tauri/src/orchestra.rs` and `scheduling.rs`; `orchestra/scheduling/` settings and queue UI |
| Native approvals          | `gatekeeper/` crate and review panel; `native_claude.rs` / `approvals_codex.rs` adapters        |
| Native questions/children | `interactions.rs`, `activity.rs`, `NativeQuestions` and `ChildSessions`                         |
| Local helper              | `local-assistant/`, native `ollama.rs`, opt-in settings and draft/title UI                      |
| Browser                   | Native `browser.rs` MCP broker/proxy and `ollama_agent.rs`; common provider invocation path     |

Scheduling caps count admissions across a run; completion frees concurrency, not total budget. Explicit Resume retains completed worker results and checks old process identity before unfinished work can restart. Closing to the tray keeps the current process alive; there is no separate daemon.

Ask-first connections and worker questions reach supported Orchestra execution paths. Gatekeeper's native protocol provider ID is `codex`, while the app uses `gpt`; account identity remains unchanged. Renderer decisions never contain executable approval inputs or arbitrary wire responses.

Local-helper failures preserve manual editing and do not invoke remote fallback. Its opt-in model/settings remain separate from the Ollama conversation model. Moli is automatic across providers; it is not controlled by the optional helper switch.

[Gatekeeper contract](gatekeeper/README.md) · [Local helper](local-assistant/README.md) · [Validation](../../docs/validation.md)
