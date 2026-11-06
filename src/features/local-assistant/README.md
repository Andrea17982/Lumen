# Local assistant (Ollama helper)

Optional, local-only text helper: automatic chat titles, plus commit messages and
handoff summaries on request. Nothing here commits, sends, or calls a remote provider.
All failures (service down, model missing, timeout, cancel) return a result and leave
the manual flow untouched.

## Integrated

- **Settings**: `localAssistant { enabled, model, autoTitles, timeoutMs }` in
  `AppSettings` (Rust `settings.rs`, TS `lib/types.ts`). Off by default. The controls
  render in Settings → Teamwork via `LocalAssistantSettings`. The Orchestra pin
  `ollamaModel` stays separate.
- **Transport**: `ollamaTransport()` calls the native `ollama_models` and
  `ollama_generate` commands (`src-tauri/src/ollama.rs`). The webview CSP blocks
  `http://127.0.0.1:11434`, so requests go through the official CLI instead. Both
  commands refuse to run while the helper is off. `generate` checks `ollama list`
  first, because `ollama run` would pull a missing model from the registry. The prompt
  travels on stdin.
- **Instance**: `instance.ts` holds one `localAssistant`, configured from settings in
  `App.tsx`.
- **Titles**: after the first reply, `nameNewChat` (`autoTitle.ts`) names the chat only
  while it still carries the title Lumen derived from its first message, and rechecks
  that after the model answers. Renamed chats are never overwritten.

## Not integrated yet

- **Commit message**: the app has no commit flow. `Draft.commit` and `DraftPanel` are
  ready for when one exists.
- **Handoff summary**: `compact_session` builds its handoff natively. A drafted summary
  would need a DraftPanel in the compact flow and a backend path that accepts edited
  text.
- `httpTransport` remains for tests only; the app does not use it.

## Tests

- `npm test`: `assistant.test.ts` (triggers, failure mapping, drafts, loopback-only
  fetch) and `autoTitle.test.ts` (derived titles, renames, first exchange only).
- `cargo test ollama`: model-name validation, terminal-sequence stripping.
- `cargo test ollama -- --ignored`: a real answer from the local daemon, plus a missing
  model refused without a pull. Needs Ollama running with one model installed.
