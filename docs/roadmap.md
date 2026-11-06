# Roadmap

## 0.3 — teamwork (released)

Both mode, cross-audit (on demand or automatic, 1–3 rounds), shared `LUMEN.md` context and file attachments, rate-limit fallback with status indicators, Rewind checkpoints, self-healing checks, token estimate, log spill, compact handoff, MCP hub with switches, local OpenAI-compatible API, and macOS/Linux process, PATH and login support. 0.3.1 polished the interface (Ask mode, overlay Settings, provider chevron). See [Teamwork features](features.md).

## 0.4 — Orchestra

- **Orchestra.** A set-and-forget switch that works with Auto. For complex tasks, one orchestrator model asks clarifying questions, splits the work into subtasks, shows the plan with the assigned agents for approval, then runs the subtasks in parallel git worktrees across all available providers and integrates the result. The orchestrator model follows the Auto preference (Budget/Default use Auto's tier with a Sonnet/Terra floor; Quality uses the strongest model). With Cross-Audit on, the provider opposite the orchestrator validates the integration (Budget) or the plan and the integration (Default/Quality).
- **Ollama.** A local provider in Connected accounts, used as the last emergency fallback and for trivially simple Orchestra subtasks.
- **Settings: Simple and Advanced.** Settings split into sections, one at a time, with a Simple/Advanced switch that hides expert options.
- **Tray and autostart.** Optional "keep in the notification area when closed" and "open at login".

See [Teamwork features](features.md#new-in-04).

## Next

- **Orchestra limits**: optional caps on sub-agents and quota per run, and smarter balancing of subtasks across accounts.
- **Ollama as a helper**: titles, commit messages and handoff summaries written locally.

## Later

- **Interactive approvals**: per-command approval cards and a command whitelist, through the CLIs' supported permission protocols.
- **Remote control**: optional Telegram bot for notifications, approvals and queued prompts.
- **Repository map**: a tree-sitter outline shared through `LUMEN.md`.
- **Screen capture** attached to the prompt.
- **Performance**: batched IPC events, a virtualized transcript, warm CLI processes, startup and memory measurements.
- **Reliability**: signed Windows and macOS builds, versioned migrations, encrypted-at-rest option.

The roadmap describes intent, not a delivery promise. Feedback and measured behavior set the priorities.
