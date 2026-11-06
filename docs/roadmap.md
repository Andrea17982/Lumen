# Roadmap

This file contains only remaining work. Remove items when implemented and verified; keep release history in [Features](features.md) and the release notes. The review order below is agreed; technical choices remain proposals until validated and discussed.

## Review and implementation order

### 1. Interface

- **Provider usage**: validate Claude's streamed rate-limit events across supported CLI versions and normal and Orchestra turns; update on Claude responses in Lumen, with no periodic requests. Validate reset countdown/zeroing, Antigravity's headless `/usage` refresh and failure handling across platforms and CLI versions. Show unavailable or stale data honestly and verify account isolation and native UI behavior on Windows, macOS and Linux.
- Test the actual Claude event-to-native-UI path and partial-window freshness. Validate Antigravity's text format against CLI versions, investigate structured output, and add representative fixtures and diagnostics for format changes beyond recognized quota rows.
- Complete the review of the shell, conversation navigation, transcript, composer, Settings, project context, Orchestra, audit, Rewind, dialogs and status feedback. Validate keyboard and screen-reader access, focus containment/restoration, narrow layouts and light/dark themes in the native webviews.
- Make conversation actions discoverable and usable without a mouse or hover; validate keyboard operation and message actions in native webviews. Keep the agreed minimal focus treatment documented in `DESIGN.md`.
- Compare relevant accessibility and platform guidance using current official sources. Agree the remaining UI changes and acceptance criteria before implementing them in `components/` or `settings/`, keeping `App.tsx` as the shell.

### 2. Command palette

- Complete the code audit of actions, menu handlers and existing shortcuts, including Ctrl/Cmd+P and Ctrl/Cmd+N. Check conflicts on Windows, macOS and Linux before choosing a palette shortcut.
- Evaluate a shared command registry for palette, menus and shortcuts, with stable identifiers, search labels, context-dependent availability and a common execution path. Preserve direct access to primary actions and existing permission checks.
- Compare React-compatible implementations against current official documentation, licenses, accessibility and native webview behavior. Agree the library, search behavior and command scope; then implement and verify keyboard navigation, focus restoration and screen-reader feedback.

### 3. Repository map

- Evaluate a local incremental symbol/reference index, including tree-sitter as a candidate. Define initial languages, invalidation for edits/deletions/renames and non-git projects, and indexing cost/size limits before choosing an implementation. Git checkpoints alone do not cover all index updates.
- Respect `.gitignore`, secret-file exclusions and workspace boundaries; define how generated/noise files are excluded. Keep generated index data separate from user-authored `LUMEN.md` notes.
- Prototype symbol search/navigation and a bounded context summary for ordinary turns and Orchestra, including text-only Ollama. Measure relevance, prompt size and indexing latency before enabling automatic injection; do not promise a complete cross-language call graph.

### 4. Context efficiency

- Complete the code audit of context construction for every integrated provider and execution path: instructions, project notes, history, attachments, tool results, audit and Orchestra handoffs. Distinguish Lumen's behavior from context management performed inside the official CLIs.
- Verify each provider's supported token usage reporting, context limits, compaction and caching against current official documentation. Label local estimates separately from reported usage and leave unavailable metrics explicit.
- Establish fixtures and baseline measurements for prompt size, relevance, latency and handoff quality. Evaluate budgets/output reserves, deduplication, truncation, compaction and cache invalidation while preserving instructions and coherent tool exchanges.
- Compare compact serialization with JSON only for metadata Lumen owns, measuring parsing reliability as well as token savings. Preserve provider-native CLI/MCP formats. Agree changes from measured results before implementation.

## Cross-platform and provider acceptance

- Validate each agreed change on Windows, macOS and Linux, across Claude Code, Codex, Gemini CLI, Antigravity and Ollama where the capability applies. Preserve official CLI authentication, read-only defaults, account isolation and stdin prompt delivery; do not assume direct provider API access.
- Complete the release path for Linux and define the supported macOS architectures (the current release builds Apple silicon only). Extend native UI verification and lint coverage for platform-specific Rust paths; record untested OS/provider combinations explicitly.
- Backward compatibility is not a requirement for the new work. Assess any obsolete compatibility paths when touched; existing migrations do not need removal merely because compatibility is optional.

## Other remaining work

- **Orchestra live validation** on an isolated copy of Lumen: concurrency and total caps, quota thresholds, provider/model switching, rate-limit cooldowns, restart and repeated Resume without duplicate effects.
- **Interactive approvals, Codex**: replace `codex exec` with the `app-server` runner for Ask-first turns (initialize, thread/turn identities, approval callbacks, delivery and recovery as described in the Gatekeeper README), then connect its adapters and cards. Until then Codex refuses Approve each action. Ask-first for Orchestra subtasks is also open.
- **Interactive approvals, native window**: check approve, reject, cancel, the countdown and the working indicator in the real webview on Windows, plus the Claude approval panel's behavior after a restart.
- **Native acceptance**: Windows clean-machine install; macOS login launch and clean install; Linux native behavior and clean install. Real macOS/Linux environments remain required.
- **Remote control**: optional Telegram bot for notifications, approvals and queued prompts.
- **Screen capture** attached to the prompt.
- **Performance**: batched IPC events, a virtualized transcript, warm CLI processes, startup and memory measurements.
- **Reliability**: signed Windows and macOS builds and an encrypted-at-rest option.

The roadmap describes intent, not a delivery promise. Update it as decisions are agreed and remove completed work.
