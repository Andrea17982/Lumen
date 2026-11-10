# Roadmap and remaining acceptance

Implemented capabilities and release history are in [Features](features.md). Detailed commands, observed results and their limits are in [Implementation evidence](implementation-status.md) and [Validation](validation.md). Completed engineering items have been removed from this checklist.

## Agreed scope

- Native direct Claude/Codex sessions are the default; Auto routing and Orchestra are explicit choices. Preserve the selected provider/model and official native authentication.
- Implement the original main sections and additional ideas. Delivered capabilities include native questions/child sessions, context/token improvements, migrations, manual Both merge, Orchestra task graph, capture shortcut, tray continuity, Godot controls, inspector, layout canvas and static worktree visual comparison. Existing Ollama title/commit/handoff helpers remain available.
- Windows is the local acceptance target. macOS universal and Linux get Actions builds/checks; real-machine non-Windows acceptance awaits testers and is not a delivery gate.
- Removed by the user: signing, Telegram, internal encryption, a separate daemon and an additional common OS filesystem jail. Use notification-area continuity, disk encryption and the official providers' sandbox capabilities. Autopilot retains full access.
- Closing the window continues work in the tray. Crash/restart recovery requires explicit Resume; no automatic replay of interrupted actions.

## Remaining delivery checks

- Complete packaging on the final source revision: Windows installer/portable, universal macOS DMG, Linux deb/AppImage. Ubuntu 24.04 is the Linux build baseline because the current capture dependency requires newer PipeWire headers than 22.04 supplies. CI has passed on Windows/macOS/Linux; packaging is tracked separately.
- The Windows packaging workflow now includes a silent install on its disposable runner and an actual installed-release MCP initialization/tool-discovery probe. Record the final result after that run completes.

## Live provider acceptance requiring the user's account/environment

- **Claude live turn:** deferred explicitly by the user after account quota exhaustion. Native initialization, controlled streaming/questions/approvals, cancellation and offline native UI paths are covered. Do not present fixtures as a live inference proof.
- **Gemini live turn:** official Gemini CLI 0.67.0 is now installed and its version/help verified. The user will complete native Google sign-in personally. After that, verify the account's real model catalog, a graph tool call, cancellation and fallback through Lumen. Configure/login through Lumen's account slot so its native account directory is used.
- **Provider/identity matrix:** extend actual multi-account and CLI-version coverage for Google-provider usage, quota parsing and fallback. Preserve unknown/stale metrics explicitly. Current Google print runners and Ollama text runner do not establish the same bidirectional UI capability set as the Claude/Codex adapters.
- **Real-limit/restart matrix:** keep expanding controlled Orchestra coverage with observed live quota cooldown, repeated Resume and interrupted native approval delivery. A real Codex Autopilot run with concurrency 1 / total 2, actual writes and clean worktree integration has passed; it does not simulate every possible crash or provider outage.

## Target-environment acceptance

- macOS/Linux native UI, login launch, clean install, accessibility and keyboard/system bindings await real testers. Linux screenshots need actual X11/Wayland runtime acceptance; a successful package/build is not capture evidence.
- Windows keyboard/focus, questions, images, inspector, canvas, merging and native tray behavior are automated. Keep real screen-reader review and alternate editor caret-position checks separate from DOM/argument tests. VS Code is installed; Cursor/Zed/Sublime are not installed here.
- Godot Play/specific-scene/Stop/Reload and MCP viewport capture have been exercised through the purchased addon in an ignored isolated Rodent Heroes copy. Runtime services were enabled explicitly only for that viewport probe and original configuration was restored. Two portrait game-window captures also passed. Physical-device review and non-Windows game/editor behavior await target devices/testers.
- The isolated Rodent chat acceptance made a bounded presentation-resource change and passed real resource/.NET/manifest checks. Authoritative gameplay-balance acceptance requires a separately defined game scenario; no balance change or live-service deployment was inferred from the Lumen task.

## Performance follow-up criteria

Keep measured regressions as fixtures before adding complexity. The Windows 1,000-message synthetic transcript loaded in approximately 0.5–0.6 s; large synthetic scroll jumps still produced an isolated approximately 0.33–0.36 s frame interval. Memoized Markdown and browser content visibility are in place. Full virtualization, Rust Markdown/ANSI rendering, compact alternate metadata serialization and warm CLI pools require evidence that they improve actual workload latency/quality without damaging native formats or coherent tool exchanges.

## Subsequent Moli request

The requested online feasibility study and Windows CLI/CDP probe are in [Moli findings](moli.md). Production automatic browser routing is not yet implemented. The user chose the official local Ollama API, with no keys or remote services, for its future tool loop; subscription providers retain official CLI authentication. The study describes the shared MCP/browser service and the limits of all-model guarantees.
