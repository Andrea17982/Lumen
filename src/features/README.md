# Integrated feature modules

The Rust host (`src-tauri/crates/lumen-core`) is the execution authority; frontend modules render state and collect scoped decisions.

| Feature                   | Host / renderer integration                                                              |
| ------------------------- | ---------------------------------------------------------------------------------------- |
| Orchestra scheduling      | `orchestra.rs` and `scheduling.rs`; `orchestra/scheduling/` settings and queue UI        |
| Native approvals          | `gatekeeper/` crate and review panel; `native_claude.rs` / `approvals_codex.rs` adapters |
| Native questions/children | `interactions.rs`, `activity.rs`, `NativeQuestions` and `ChildSessions`                  |
| Browser                   | `browser.rs` MCP broker/proxy and `ollama_agent.rs`; common provider invocation path     |

Scheduling caps count admissions across a run; completion frees concurrency, not total budget. Explicit Resume retains completed worker results and checks old process identity before unfinished work can restart. Turns run in the local service (`lumen-daemon`), so closing the window to the tray does not stop them.

Ask-first connections and worker questions reach supported Orchestra execution paths. Gatekeeper's native protocol provider ID is `codex`, while the app uses `gpt`; account identity remains unchanged. Renderer decisions never contain executable approval inputs or arbitrary wire responses. Moli is automatic across providers.

[Gatekeeper contract](gatekeeper/README.md) · [Validation](../../docs/validation.md)
