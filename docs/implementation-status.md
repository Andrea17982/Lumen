# Roadmap implementation evidence - 10 November 2026

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
- Release workflow extended for Linux deb/AppImage and universal macOS; Unix clippy/fmt checks. Branch CI and manual packaging runs started on 10 November (42794155852 / 42794156400); final results are recorded below when observed.

## Verification already observed

- Real Codex (`gpt-6.1-sol`): structured question → user response → actual file creation → follow-up resumed the same native thread; passed in ~27 s total for two turns. Auth stayed in its existing official CLI account directory, through a temporary junction, without reading/copying credentials.
- Real installed LSP servers: Rust analyzer 1.99.1, typescript-language-server with the project's TypeScript 5.9 runtime, and csharp-ls 0.28.0 all detected actual type errors. Fixed diagnostic refresh/retrigger handling and equivalent Windows file-URI normalization. TypeScript 7 no longer provides the tsserver.js expected by typescript-language-server; the explicit 5.9 runtime was used, not a false success.
- Functional Rust coverage now includes 124 passing tests and 12 opt-in tests excluded from the ordinary run. Individually requested live/metadata probes are recorded separately below. Frontend: 60 unit tests and 27 Playwright tests passed. Separately invoked native probes are not included in these counts.

- Installed Claude acknowledges native stdio initialize without any model prompt (~1.1 s in local probe).
- Full offline Windows native smoke passed, including native question-answer UI round trips, normal/write/Both/Orchestra/rewind/cancel/rate-limit paths, manual merge with stale-preview rejection, Godot opt-in/wrong-editor rejection, static worktree preview server lifecycle and capture-shortcut registration/reset without an accidental capture.
- Image load/zoom/Escape/focus and inspector navigation/address-boundary Playwright tests passed.
- Real Codex Orchestra on an isolated project passed with only one GPT account: Autopilot, two independent writing tasks, concurrency cap 1 / total cap 2, both actual files integrated, worktrees cleaned, no second approval. It exposed/fixed an old routing bug where an explicit GPT architecture request incorrectly selected Claude. The coordinator also now respects the selected model. Observed run: ~63 s.
- Real Rodent Heroes isolated-copy chat acceptance changed only `game/scenes/components/resource-bar.tscn` separation 4 to 6, through Lumen/Codex. Lumen's actual `res://` check passed (ignored missing local config reported honestly), and .NET built with zero errors/warnings. This is a bounded presentation-resource edit, not gameplay-balance or device acceptance. The original checkout remained untouched.
- The project's own `python tools/sync-client-art.py --check` verified 373 approved files. Intentionally corrupting one SHA failed with the expected mismatch; the manifest was restored byte-for-byte. Godot headless import of the isolated project also succeeded.
- An actual C# compiler error (`CS0029`) in a disposable folder with spaces produced a source link to Probe.cs:1 in Lumen. Added coverage for unquoted absolute compiler paths containing spaces.
- A native Windows WebView2 transcript of 1,000 synthetic messages loaded in ~599 ms. Median measured animation-frame interval was 11.9 ms, with one 361 ms worst interval during synthetic jumps. This does not establish smoothness for every project/transcript. Historical Markdown is memoized; native formats and React links/images remain intact. No measured case currently justifies moving Markdown to Rust or adding a new warm-process lifecycle.
- Google graph configuration tests preserve unrelated servers/settings, prove stable repeated configuration, remove only Lumen's own entry when disabled and reject malformed native JSON. Per-process workspace context avoids cross-project config races. These tests are not a live Gemini model-turn proof.
- Layout tests cover keyboard movement, save/reload/removal and changed/unchanged/removed context on native resume. LSP regression coverage now reproduces retrigger, refresh and equivalent encoded file URIs while preserving concrete error evidence.
- Moli v1.1.23 Windows feasibility probe passed DOM/forms/JS and screenshot tests through CDP. Full findings, benchmark limits and the unimplemented production-integration boundary are in [moli.md](moli.md).
- Real Godot 4.7.2 LSP with an owned headless editor detected an actual GDScript type error at probe.gd:3:19; the temporary editor was stopped afterward. Rust, TypeScript, C# and Godot installed-server interoperability have all been observed locally.
- Actual Godot MCP Pro 1.17.1 in the ignored Rodent acceptance copy passed Discover, Play of a specific res:// scene, Stop and Reload through Lumen. This exposed/fixed the editor reconnect wait and the actual string-valued play mode schema. A separate application identity isolated its own connection token; the harness never read or copied the token. Original project/MCP configuration was restored and owned editor/game processes stopped. Runtime services stayed disabled.
- Lumen captured actual isolated game windows requested at 360×640 and 720×1280. Window images measured 355×654 and 715×1294 including desktop/chrome effects. These are desktop proofs, not physical-device or exact viewport captures. A direct MCP viewport capture requires the project's explicitly enabled MCPScreenshot runtime autoload; native window capture remains available without it.
- A subsequent explicit viewport probe enabled runtime services only in that isolated copy, then captured a genuine 1137×640 viewport through Lumen's MCP image path. Discover/Play/Capture/Stop/Reload all passed; the original project/MCP configuration was restored afterward. The original Rodent checkout and its token were untouched.
- User decision on 10 November: install Gemini CLI and let the user complete native sign-in; use the official loopback Ollama API for the future Moli tool loop, with no cloud services or keys. Gemini CLI 0.67.0 was installed globally using the official npm package; version/help passed, authentication remains the user's step.
- CI passed on Windows, macOS and Ubuntu for 574c6b7; it also passed for fb91e6d on the pinned Ubuntu 24.04 runner. Ubuntu 22.04 packaging failed because its PipeWire headers do not match libspa 0.9.2; packaging was moved to 24.04, consistently with CI. Final packaging results are recorded when observed.

- A real Codex turn used the configured graph MCP `find_symbol` tool and returned the actual src/lib.rs location. Tool activity, not only assistant prose, confirmed the call.
- The owned-metadata probe round-tripped 100 synthetic task rows including quotes/Unicode/newlines exactly. Keyed JSON: 25,893 bytes / 6,509 local tokens / ~1,306 µs serialize+parse; fixed-schema tuples: 16,983 bytes / 4,408 tokens / ~708 µs. This debug-build synthetic result demonstrates ~32% token savings for this dataset, not model-generation reliability. Keep keyed/provider-native JSON: the sub-2-ms local serialization cost is not the dominant measured latency, native resume already avoids replaying that state, and tuple ordering would require a new generation/versioning contract. Repeat with `cargo test ... owned_metadata_serialization_probe -- --ignored --nocapture`.
- Preview HTTP parsing now handles fragmented request headers with an 8 KB bound; a focused regression reproduces the partial-first-packet case.
- Codex child-agent completion messages are preserved in the child body rather than expanded into its title; repeated native completion events do not duplicate the saved output. The installed official CLI reports its stable multi_agent feature enabled in both the normal and Lumen account profiles.

## Remaining work / acceptance

- Await final-revision Actions packaging and clean-runner installer results, then record artifact evidence. Local Windows installer/portable creation and the release executable's MCP initialization/tool discovery have passed.
- The user will complete Gemini native sign-in; its real model turn cannot be claimed yet. Claude live inference remains explicitly deferred. Additional account/version/native-OS matrices are listed in [Roadmap](roadmap.md).
- Real screen-reader, alternate uninstalled editor caret-position and physical-device/non-Windows UI acceptance remain separate from automated DOM, argument, build and desktop proofs.
- Moli production routing remains the subsequent integration work described in the requested feasibility study; its Ollama local-API architecture choice is now agreed.
