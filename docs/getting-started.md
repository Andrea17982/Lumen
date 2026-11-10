# Getting started

## Before you begin

Lumen runs installed, unmodified provider CLIs. You need access to Claude Code or Codex under your own provider agreement, an internet connection for inference, and a trusted local project directory. An account slot is metadata, not an authenticated identity until official sign-in succeeds.

Install the Windows x64 alpha from [Releases](https://github.com/Andrea17982/Lumen/releases), or follow the source-build commands in [README](../README.md). Restart Lumen after installing a CLI so it inherits the new PATH. Use a current official CLI release supporting the flags in [development](development.md).

## First conversation

1. Open Settings using the top-right sliders button or the account control in the sidebar.
2. Under Connected accounts choose +. Enter a label and provider, then Add account.
3. Choose Sign in. Finish authentication in the terminal opened for that account. Lumen never asks you to paste a token. Close the terminal when done.
4. Choose the shield button beside the account to query the official CLI login status.
5. Choose Workspace folder. Use a project directory you trust, not your entire user profile.
6. Return to the conversation and type a request. A native direct provider is selected by default, preserving your model/effort choice. Auto routing and Orchestra are explicit options. The first message supplies an initial local title; the optional Ollama helper can refine it.
7. Expand activity to inspect exposed reasoning summaries, tools, edits, planning or errors.

**Ask** (read-only) is selected for each app launch. Both mode is available only in Ask. Enabling File edits on allows the provider's configured file tools or workspace-write sandbox. The Permissions selector also offers Autopilot, which disables CLI restrictions/approval prompts and enables available tools under your OS account. The setting applies to subsequent turns; it does not change an in-flight process. Workers used for review and parallel analysis are read-only. Multi-stage requests can consume several inference calls.

## Everyday controls

| Action | Control |
| --- | --- |
| New conversation | Sidebar +, or Ctrl/Cmd N |
| Send / new line | Enter / Shift Enter |
| Stop generation or a queued turn | Square button in the composer |
| Rename, pin, delete | Conversation header … menu |
| Search | Sidebar search field; title search only |
| Load older sessions | Load more conversations after each 100 rows |
| Manual model/account/effort | Select Claude or GPT, then expand the chevron |
| Close settings / mobile sidebar | Escape |

Use Workspace and the adjacent folder picker beside the AI selector to bind the selected conversation to a named workspace and a specific working folder. Folder changes are disabled while the conversation has an active/queued turn. Legacy conversations retain their folder after migration. The default folder and appearance live in webview local storage; messages and account metadata live in SQLite.

## Recovery

- **CLI not found:** install its official release, check `claude --version` / `codex --version`, restart Lumen.
- **Not signed in:** use Sign in for that exact account slot, then check connection. Your pre-existing terminal login is intentionally not imported.
- **Model not available:** clear a manually set model to use the provider default, or choose a model your account can access. Auto's Claude aliases require matching plan access.
- **Quota / rate limit:** wait or change provider manually. Lumen does not rotate accounts to evade limits.
- **Tool denied:** read-only and provider permissions may deny tools. Enable file edits only when appropriate. Claude shell tools are enabled only with Autopilot.
- **Stuck work:** stop the turn. If you restart after a crash, previously running/waiting sessions become interrupted. Partial text is checkpointed roughly every 750ms when events arrive; the latest chunk may be lost.
- **Linux login:** see [Accounts](accounts.md); the automatic terminal launcher currently targets Windows.

Deleting a conversation permanently removes its local messages; it does not delete CLI-owned history. Account removal asks for confirmation and deletes its local directory. It does not revoke the provider's remote authorization. See [Security](../SECURITY.md).
