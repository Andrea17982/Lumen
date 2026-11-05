# Workspace, models and permissions — 0.2

## Conversation location

The composer contains a Workspace selector and an adjacent folder button beside Auto / Claude / GPT. Choose Add workspace to select a folder and create a named workspace (initially named after that folder). Select an existing workspace to bind the conversation to its default folder. Use the folder button to select a different working folder for that conversation while keeping the workspace association.

The header shows the named workspace; the folder control exposes the full path in its tooltip. Each conversation persists both association and folder independently. Changing one conversation does not move another conversation. The last selected location is used for new conversations. Queued/running turns disable location changes; the backend also rejects them. Existing 0.1 conversations retain their folders and messages through a SQLite migration.

## Accounts

In Settings, use + to add, the pencil to rename, the shield to check connection, Sign in for official CLI login, and the trash control to remove. Removal still asks before deleting local account files. Renaming changes only the display name, preserving the UUID, isolated configuration folder and login state.

When Claude or GPT is selected, Account also has a pencil for inline rename and a Settings button for account management. Switching providers opens these optional controls. Settings preserves your model choice when returning to the composer.

## Official model and reasoning catalogs

Model is a dropdown, never a free-text field. Lumen queries the chosen account's installed CLI:

- **Codex:** initialize the official app-server over stdio, then call `model/list`, including pagination. The model's `supportedReasoningEfforts` and `defaultReasoningEffort` drive Reasoning.
- **Claude Code:** initialize the official CLI streaming control protocol (the protocol used by Agent SDK `supportedModels`) and consume its returned `models`. `supportedEffortLevels` drives Reasoning. If no default level is supplied, Lumen offers Provider default and omits the effort flag.

The lists are cached per account for five minutes. The refresh button queries again; changing account reloads the list. Failed or empty discovery shows a recoverable error, not a fabricated list. Changing model rebuilds the reasoning choices. The backend rejects a manually requested model/effort combination absent from the catalog. Auto resolves each stage to a supported effort and records the effective choice in activity.

A returned catalog is not an entitlement guarantee. Provider plan, organization policy, authentication and per-request availability still apply. Discovery initializes metadata only: it does not submit a prompt or perform inference, and Lumen does not read credential contents.

Sources: [OpenAI app-server model/list](https://learn.chatgpt.com/docs/app-server), [Claude SDK ModelInfo](https://code.claude.com/docs/en/agent-sdk/typescript), [Claude CLI flags](https://code.claude.com/docs/en/cli-reference).

## Permissions

| Mode | Claude Code | Codex |
| --- | --- | --- |
| Read only | Restricted Read/Glob/Grep; no Bash; strict MCP override | Read-only sandbox, approvals never |
| File edits on | Restricted Read/Glob/Grep/Edit/Write; no Bash; strict MCP override | Workspace-write sandbox, approvals never |
| Autopilot | `--dangerously-skip-permissions --tools default`; no restricted/strict-MCP override | `--dangerously-bypass-approvals-and-sandbox`; no sandbox override |

Autopilot grants full access to the CLI's available tools, including filesystem changes, commands, network and configured computer-automation extensions, without CLI approval prompts. It applies to subsequent writer/synthesis turns. Review and parallel-analysis workers remain read-only. Read only remains the startup default.

Autopilot does not install a GUI automation engine or grant administrator privileges. Operating-system ACLs, UAC, provider-managed policy and tool availability still apply. Actual screen/mouse interaction requires a compatible automation tool, MCP extension or executable available to the provider. A failed capability is reported as a provider failure rather than claimed as successful PC control.

No live unrestricted system operations were used to validate this feature. Tests verify official flag mapping and execute offline CLI fixtures. Real account inference, GUI automation and provider permission enforcement require acceptance testing.
