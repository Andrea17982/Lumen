# Development changes after 0.6.0

The published 0.6.0 tag is unchanged. These changes are on `codex/native-sessions-roadmap`; build/test evidence identifies the tested source revision separately from later documentation updates.

## Native sessions and execution

- Native direct sessions are the default; the explicit provider choice is persisted. Auto and Orchestra are optional. An architecture/security classification can no longer override an explicit GPT selection or require an unrelated Claude account.
- Claude uses its bidirectional stream-json control protocol, with initialization, native Agent tools, user questions, cancellation and a persistent stdin writer. Codex uses app-server in normal and Orchestra runs, with cached installed schemas and per-turn cwd/permission settings.
- Native user questions support options, multiple selection, custom answers and secret input. Closed/cancelled requests reject stale answers. Public clarifications survive replay and Compact; secret answers are excluded from the durable decision text.
- Native child-agent output is displayed separately and compacted in memory/storage, preserving questions and checkpoints during long streams. Images and project/web links are displayed through bounded preview/open commands, with keyboard focus restoration.
- Autopilot preserves the native tool set without the extra Claude deny list or repeated plan gates. Orchestra respects the selected coordinator model and carries actual approval/question connections into workers. Default direct runs avoid extra classifier/planner turns.
- Native resume sends only the new request, current permissions, attachments and changed/removed layout constraints. Full replay is constructed only when needed for a missing native session or provider fallback. CLI-native instructions/authentication remain intact.

## Context, diagnostics and performance

- Additional Lumen validation is opt-in; native agents retain their ordinary build/test tools. Orchestra retains final checks.
- Text estimates use a real o200k tokenizer and are debounced; provider-reported usage stays separate from local estimates. Complete pasted lockfiles are blocked. Images are explicitly not counted.
- Text deltas are coalesced in Rust before IPC at 24 ms, flushing before control events. Historical Markdown is memoized while preserving React link/image behavior and provider-native formats.
- Google graph MCP configuration uses stable per-account entries and per-process workspace context, preserving other native settings and preventing concurrent project-path overwrites. It supports native Ask-first Claude/Codex paths too.
- LSP diagnostic refresh/retrigger handling and equivalent Windows file URI normalization were corrected. Published and pulled diagnostics retain concrete error evidence independently. Actual compiler source links now handle unquoted absolute paths containing spaces.
- Synthetic measurements cover a 1,000-message native transcript and keyed JSON versus fixed-schema tuples. Compact tuples saved tokens on the synthetic dataset, but native/keyed formats are retained because model-generation reliability has not been established and local serialization was not the dominant measured cost.

## Project tools

- The inspector provides sandboxed localhost web preview, pixel zoom/grid, bounded read-only JSON/SQLite inspection, project changes/checks, source diffs, screenshot comparison and a layout canvas.
- Layout blocks support pointer/keyboard movement and bounded geometry. The project-owned layout spec is persisted atomically and sent as changed context to native sessions.
- Both has per-file manual merge choices and editable text, backed by immutable Git trees and a private index. It preserves user staging/unrelated changes and rejects stale previews. An unchanged/removed second lane uses the actual common baseline.
- Retained Both/Orchestra worktrees can serve built static web output side by side. Owned loopback servers stop when the view closes. HTTP request headers are bounded and handle fragmentation; project commands are not executed by the preview server.
- Orchestra shows its task specification, dependencies and drag/keyboard agent assignments. Each writing task retains its isolated worktree.
- Godot visual controls are opt-in and verify the connected editor root before an action. Actual advertised schemas are used, including Godot MCP Pro's string-valued specific-scene mode. A fresh MCP server waits for the editor reconnect before sending an action.
- Godot Play/Reload/Stop/viewport capture were exercised against the purchased addon in an ignored isolated copy. Runtime capture requirements are explicit; native window capture is available separately. No purchased addon is redistributed.

## Storage and background work

- SQLite schema upgrades are transactional/versioned, with rollback and future-version rejection.
- Closing to the notification area is the default. Crash/restart states are interrupted and require explicit Resume. There is no separate daemon.
- The optional global capture shortcut attaches an image to the composer. Registration/reset is tested and does not take a screenshot by itself.
- The other chat's `/workspace` and `/compact` autocomplete changes are retained.

## Builds and evidence

- Windows NSIS/portable, universal macOS Intel/Apple silicon DMG and Linux deb/AppImage packaging are configured in Actions. Ubuntu 24.04 supplies the PipeWire headers required by the current capture dependency. The Windows workflow includes silent installation on its clean runner and an installed-release protocol probe.
- Signing, Telegram, internal encryption and a common extra OS jail were removed by the user's decision. Disk encryption, native provider sandbox capabilities and tray continuity supply the agreed alternatives.
- Local proof includes native UI fixtures, real Codex questions/resume/graph use, real Codex Orchestra writes/caps/integration, actual Rust/TypeScript/C#/Godot LSP errors, C# compiler links, real Godot editor actions and screenshots, plus the Rodent manifest corruption/restoration check. [Evidence](../implementation-status.md) records exact scope and remaining external/live acceptance.
- Gemini CLI 0.67.0 is installed on the development machine; the user completes native sign-in. Claude live inference remains deferred as agreed. Physical-device, native non-Windows UI, screen-reader and uninstalled-editor acceptance are not implied by compilation or DOM tests.

## Moli browser integration

- Automatic shared `lumen_browser` MCP tools for Claude, Codex, Gemini and Antigravity, including native resume, fallback and Orchestra workers. Official native authentication is retained; supported built-in browsing alternatives are disabled/denied.
- Ollama uses its official loopback API for model discovery, helper text and a real tool/result loop. Native function calling and a constrained JSON-action fallback share Moli and the same permission enforcement. All installed conversational models are selectable. Remote endpoints/cloud aliases are rejected; no keys or automatic model downloads.
- Pinned Moli v1.1.23 resources and upstream notices are bundled with digest verification for Windows/macOS/Linux. Windows portable delivery is a ZIP containing its required resources.
- Markdown fetch/search, persistent CDP navigation/extraction, click/fill/evaluate and screenshots; isolated contexts per conversation/worker/workspace; cancellation, deadlines, bounded output and one-action mutation permissions in Ask mode. Autopilot proceeds directly.
- Browser activity and screenshots appear in the existing transcript. Worker screenshots remain in the conversation project after worktree cleanup. Unsupported/blocked sites report an error instead of silently switching browsers.
- Targeted real-engine tests passed, plus real Codex browsing/screenshot/native resume and an actual local Ollama model consuming a fetched page. [Evidence and compatibility limits](../moli.md) distinguish these results from deferred Claude/Google live acceptance and from model-intelligence claims.
