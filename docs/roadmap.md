# Roadmap

## 0.3 — teamwork (this release)

Both mode with worktree isolation, cross-audit (on demand or automatic, 1–3 rounds), shared `LUMEN.md` context and file attachments, rate-limit fallback with status indicators, Rewind checkpoints, self-healing checks, token estimate, log spill, compact handoff, MCP hub with switches, local OpenAI-compatible API, and macOS/Linux process, PATH and login support. See [Teamwork features](features.md).

## Next

- **Interactive approvals**: per-command approval cards and a command whitelist, through the CLIs' supported permission protocols (Claude permission prompt tool, Codex app-server) instead of headless profiles.
- **Remote control**: optional Telegram bot, used only by an authorized user ID, for notifications, approvals and queued prompts.
- **Repository map**: a tree-sitter outline of signatures with line ranges, shared through `LUMEN.md`.
- **Plan and run**: split a request into a checklist and assign tasks to Claude or ChatGPT in parallel worktrees.
- **Screen capture**: a region capture, resized for vision models, attached to the prompt.
- **Local helper model**: optional Ollama for titles, commit messages and handoff summaries.
- **Performance**: batched IPC events, a virtualized transcript, warm CLI processes, and memory and startup measurements.
- **Reliability**: signed Windows and macOS builds, versioned migrations, encrypted-at-rest option, Linux CI.

The roadmap describes intent, not a delivery promise. Feedback and measured behavior set the priorities.
