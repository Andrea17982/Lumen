# Roadmap

This file contains only remaining work. Remove items when implemented and verified; keep release history in [Features](features.md) and the release notes. The review order below is agreed; technical choices remain proposals until validated and discussed.

## Review and implementation order

### 1. Interface

- **Provider usage**: validate Claude's streamed rate-limit events across supported CLI versions and normal and Orchestra turns; update on Claude responses in Lumen, with no periodic requests. Validate reset countdown/zeroing, Antigravity's headless `/usage` refresh and failure handling across platforms and CLI versions. Show unavailable or stale data honestly and verify account isolation and native UI behavior on Windows, macOS and Linux.
- Test the actual Claude event-to-native-UI path and partial-window freshness. Validate Antigravity's text format against installed CLI versions and investigate whether it offers structured output.
- Complete the review of the shell, conversation navigation, transcript, composer, Settings, project context, Orchestra, audit, Rewind, dialogs and status feedback. Validate keyboard and screen-reader access, focus containment/restoration, narrow layouts and light/dark themes in the native webviews.
- Validate compact Usage rows, status details, the context indicator popover and the larger-text preference in native webviews. Validate conversation and message actions with keyboard and screen readers, including focus restoration. Keep the agreed minimal focus treatment documented in `DESIGN.md`.
- Verify model-requested screen capture in the native Claude Ask-first flow: tool discovery, monitor/window choice, returned image, saved screenshot, cancellation/error handling and no capture without a tool call. Expand to Codex/other supported providers only through their documented tool protocols.
- Verify Simple mode (Orchestra toggle only), Advanced disclosure and the Restore defaults flow in the native webview, including API shutdown/autostart reset and preservation of accounts/conversations.
- Compare relevant accessibility and platform guidance using current official sources. Agree the remaining UI changes and acceptance criteria before implementing them in `components/` or `settings/`, keeping `App.tsx` as the shell.

### 2. Command palette

- Check Ctrl/Cmd+K, Ctrl/Cmd+N, Ctrl/Cmd+P and `/` against macOS and Linux webview and system bindings. Verify keyboard navigation, focus restoration and screen-reader feedback for the palette and the conversation menu in the native webviews. The palette is custom (no library); revisit only if accessibility testing shows gaps.

### 3. Repository map

- The map uses per-language patterns (decided over tree-sitter for now: no native grammars to build. On Rodent Heroes, 289 files and 3,234 declarations, a debug build indexed in 2.1 s from a cold disk, 155 ms with a warm file cache and 40 ms when nothing changed; a request naming three declarations added 301 characters in 31 ms). Revisit tree-sitter only if missed or wrong declarations are found on real projects; record them as fixtures first.
- Orchestra subtasks receive the orchestrator's plan, not their own map block; measure whether subtask prompts benefit before adding it.

### 4. Context efficiency

- Complete the code audit of context construction for every integrated provider and execution path: instructions, project notes, history, attachments, tool results, audit and Orchestra handoffs. Distinguish Lumen's behavior from context management performed inside the official CLIs.
- Context indicator: Claude's window and CLI overhead are measured from its own reports. Codex reports turn totals (`turn.completed` usage) but not a per-request size or window; Gemini CLI, Antigravity and Ollama report neither in Lumen yet. Capture what each CLI offers and confirm the published windows used meanwhile. Image attachments are not counted.
- Verify each provider's supported token usage reporting, context limits, compaction and caching against current official documentation. Label local estimates separately from reported usage and leave unavailable metrics explicit.
- Establish fixtures and baseline measurements for prompt size, relevance, latency and handoff quality. Evaluate budgets/output reserves, deduplication, truncation, compaction and cache invalidation while preserving instructions and coherent tool exchanges.
- Compare compact serialization with JSON only for metadata Lumen owns, measuring parsing reliability as well as token savings. Preserve provider-native CLI/MCP formats. Agree changes from measured results before implementation.

## Game development and optional Godot integration

Rodent Heroes is the first acceptance project. Integrations stay optional and project-scoped, without bundling the purchased addon or reading tokens into prompts/logs.

Verified on Windows, 2026-11-07: Lumen started the project's `godot-pro` server (Godot MCP Pro 1.17.1, 187 tools) and read `get_project_info` from the open editor (Rodent Heroes, Godot 4.7.2, 360×640); with the editor closed it reported "not connected". Claude Code 2.1.314 and Codex 0.167.0-alpha.3.2 each called `get_project_info` in the project. Repository-map indexing, the `res://` check, the backend tests (270), simulation regressions (17) and the Godot .NET build ran through Lumen's check runner on an isolated clone.

1. **Godot connection**
   - Codex `exec` turns decline project MCP tool calls unless the tool is pre-approved (`mcp_servers.<name>.tools.<tool>.approval_mode="approve"`). Decide whether Lumen pre-approves a reviewed list of read-only Godot tools for Codex turns, or leaves this to the project's `.codex/config.toml`.
   - Test the untrusted-project state for Claude and Codex; repeat on macOS/Linux.
2. **Repeatable project checks**
   - Confirm that a real Godot or C# compile error links to its source (the `res://` check and `project.godot` links are verified; a compiler error is not yet).
   - Verify line-level opening in VS Code, Cursor, Zed and Sublime Text in the native Windows window and on macOS/Linux.
3. **Visual review**
   - Verify user-started capture of the native window, window capture of the running game and import on two portrait resolutions; record manual device review separately. Capture was verified through `xcap` directly (2560×1600 screen and one window), not yet from the app.
4. **Asset and content integrity**
   - Run the project's own art/manifest check (`python tools/sync-client-art.py --check`) on the isolated copy with an intentionally changed manifest.
   - Show scene/resource diffs inside Change review instead of only listing the files; reuse Rewind for undo rather than another checkpoint mechanism.
5. **End-to-end acceptance on Rodent Heroes**
   - In an isolated copy, complete one bounded gameplay change driven from a chat, its project checks and one visual comparison; record commands and results. Retain macOS/Linux and other provider checks as separate open acceptance work. No deployment, paid generation or implicit export.

## Cross-platform and provider acceptance

- Validate each agreed change on Windows, macOS and Linux, across Claude Code, Codex, Gemini CLI, Antigravity and Ollama where the capability applies. Preserve official CLI authentication, read-only defaults, account isolation and stdin prompt delivery; do not assume direct provider API access.
- Complete the release path for Linux and define the supported macOS architectures (the current release builds Apple silicon only). Extend native UI verification and lint coverage for platform-specific Rust paths (screen capture on Linux needs X11 or a Wayland portal); record untested OS/provider combinations explicitly.
- Backward compatibility is not a requirement for the new work. Assess any obsolete compatibility paths when touched; existing migrations do not need removal merely because compatibility is optional.

## Other remaining work

- **Orchestra live validation** on an isolated copy of Lumen: concurrency and total caps, quota thresholds, provider/model switching, rate-limit cooldowns, restart and repeated Resume without duplicate effects.
- **Interactive approvals**: Ask-first for Orchestra subtasks. Codex file-change callbacks are covered by fixtures only; a real model turn raised a command approval. Check approve, reject, cancel, the countdown and the working indicator in the real webview on Windows, and the approval panel after a restart.
- **Native acceptance**: Windows clean-machine install; macOS login launch and clean install; Linux native behavior and clean install. Real macOS/Linux environments remain required.
- **Performance**: streamed events are applied once per frame and earlier messages skip layout (`content-visibility`) until they scroll into view. Remaining: coalescing deltas in Rust before IPC, measuring long-transcript scrolling in the native webviews (a full virtualized list only if that shows a need), warm CLI processes, startup and memory measurements.
- **Reliability**: signed Windows and macOS builds and an encrypted-at-rest option.

## Future

- **Remote control**: optional Telegram bot for notifications, approvals and queued prompts.

The roadmap describes intent, not a delivery promise. Update it as decisions are agreed and remove completed work.
