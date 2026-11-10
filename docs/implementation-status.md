# Roadmap implementation evidence — 9 November 2026

This is the active work record, not a completion claim. Preserve user changes made by the other Lumen chat (composer slash autocomplete).

## Decisions

- Main roadmap and previous ideas are in scope.
- Native direct sessions by default; Orchestra explicitly enabled.
- Local acceptance on Windows. macOS universal (Intel + Apple silicon) and Linux packages/checks via GitHub Actions; real testers later.
- Removed by user: signing, Telegram, internal encryption, separate daemon. Background continuity uses notification area; crash/restart recovery is explicit.
- No additional common filesystem jail. Preserve native provider sandbox capabilities; Autopilot has full access.
- Claude's live model-turn test is deferred by the user's explicit choice after the CLI reported an exhausted quota. The initialized protocol and complete fixture/UI path remain tested.

## Implemented; final integration verification still in progress

- Bidirectional native Claude stream-json, initialize handshake, stdin kept open, native user questions with radio/checkbox/free text, cancellation and stale request rejection, existing approval journal for Ask-first, screenshot MCP preserved. Native tools/Agent enabled, no extra Autopilot deny list.
- Codex app-server in all ordinary/Orchestra modes with current permission/cwd per turn, thread resume, user questions, cached installed schema evidence, native image/agent/token-usage events.
- Direct provider is the default. Native resume sends new request/permission changes, not the full ledger; replay is built only on actual missing-session errors or fallback. Additional Lumen validation is opt-in; Orchestra final checks remain.
- Image previews/zoom, file and web links through backend validation, historic workspace association. Remote images load on user action.
- Native child-agent disclosures, compact persisted child streams, durable worker questions in Orchestra messages.
- Real local o200k tokenizer, pasted-lockfile detection, Rust delta coalescing at 24 ms with ordered control events.
- Versioned atomic SQLite migrations with newer-schema rejection and rollback tests.
- Inspector dock: sandboxed localhost web preview, pixel image zoom/grid, bounded read-only JSON/SQLite, project changes/checks, screenshot comparison. Scene/resource text diffs.
- Layout canvas: pointer/keyboard positioning, editable block roles/geometry/constraints, project JSON spec; native sessions receive changed/removed specs as context deltas.
- Optional global capture shortcut; no capture at registration. Notification-area continuation is the default.
- Manual Both merge: immutable Git trees, per-file choices/manual text, private index, checked patch, preserved user index and unrelated changes.
- Orchestra task specification and dependency graph with drag assignment and keyboard selectors.
- Release workflow extended for Linux deb/AppImage and universal macOS; Unix clippy/fmt checks. Remote workflows not run yet.

## Verification already observed

- Real Codex (`gpt-6.1-sol`): structured question → user response → actual file creation → follow-up resumed the same native thread; passed in ~27 s total for two turns. Auth stayed in its existing official CLI account directory, through a temporary junction, without reading/copying credentials.
- Real installed LSP servers: Rust analyzer 1.99.1, typescript-language-server with the project's TypeScript 5.9 runtime, and csharp-ls 0.28.0 all detected actual type errors. Fixed diagnostic refresh/retrigger handling and equivalent Windows file-URI normalization. TypeScript 7 no longer provides the tsserver.js expected by typescript-language-server; the explicit 5.9 runtime was used, not a false success.
- Latest complete Rust run: 114 passed, 10 ignored live tests. Latest complete Playwright run: 26 passed. Additional features/regression tests are still being added, so rerun at completion.

- Installed Claude acknowledges native stdio initialize without any model prompt (~1.1 s in local probe).
- Full offline Windows native smoke passed after adapting fixture protocols, including both native question-answer round trips through the real UI and normal/write/Both/Orchestra/rewind/cancel/rate-limit paths. More features were added afterward, so rebuild and rerun before completion.
- Image load/zoom/Escape/focus and inspector navigation/address-boundary Playwright tests passed.
- Last full Playwright run: 24/26 passed; two old expectations (default native catalog check vs preview, Orchestra badge only in Auto) were corrected afterward; rerun required.
- SQLite inspection and manual merge unit tests passed. Latest complete Rust suite still needs a rerun after the current changes. One expected default-tray assertion was updated.

## Remaining work / acceptance

- Finish and test Google-provider graph/native protocol capability handling (Gemini CLI absent; Antigravity and Ollama installed).
- Finish Godot opt-in Play/Reload/capture controls and real isolated Rodent Heroes acceptance, compiler/source links, manifest failure fixture and portrait captures.
- Real LSP interoperability with installable/configured servers, live native question/approval acceptance and isolated real Orchestra caps/resume tests.
- Finish frontend/backend persistence/cancellation checks, snapshot resume/layout deltas, private-index/stale-merge tests, capture shortcut registration/reset checks and inspector/canvas/graph native visual review.
- Measure context/token/IPC/rendering/startup/long transcript behavior; decide Markdown/ANSI placement from evidence, not assumption. Avoid adding speculative virtualization/warm-process complexity without a measured reason.
- Complete documentation, full checks, Windows build, GitHub Actions build/packaging verification. Do not mark unavailable real macOS/Linux testing or physical-device review as passed.
