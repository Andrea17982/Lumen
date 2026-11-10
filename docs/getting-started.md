# Getting started — 0.7

## Install

Download from [GitHub Releases](https://github.com/Andrea17982/Lumen/releases/latest). Windows offers an installer or a portable ZIP; extract the entire ZIP, including `resources/moli`. macOS uses a universal Intel/Apple silicon DMG. Linux offers DEB/AppImage built on Ubuntu 24.04. Packages are unsigned; checksums accompany the release. Moli is bundled.

Install the official CLI for each subscription provider you want to use, or start Ollama with an installed local model. Restart Lumen after changing PATH. You need your own authorized provider access and a trusted project directory. [Account setup](accounts.md).

## First conversation

1. Open Settings → Accounts and add a named provider slot.
2. Use Sign in and finish the provider's official terminal/browser login. Lumen never asks you to paste credentials. Ollama needs no sign-in.
3. Check the account connection, choose a project folder and return to the conversation.
4. Select a direct provider; expand its options to choose the available model/reasoning level.
5. Select permissions, type a request and send. Direct sessions are the default; Auto and Orchestra are explicit choices.
6. Expand activity for exposed tools, browser work, checks and provider summaries. Native questions appear as structured controls.

**Read only** is the initial permission profile. **File edits**, **Autopilot** and **Approve each action** have different semantics and provider support; see [Permissions](workspace-models-permissions.md). Autopilot authorizes available tools under your OS account without additional Lumen plan-approval loops. It does not grant administrator privileges.

## Everyday controls

| Action | Control |
| --- | --- |
| New conversation / project | Sidebar +; Ctrl/Cmd+N / Ctrl/Cmd+P |
| Command palette | Ctrl/Cmd+K |
| Send / new line | Enter / Shift+Enter |
| Stop active or queued work | Square button in the composer |
| Rename, pin, move, delete | Conversation menu or context actions |
| Search | Sidebar search; title search |
| Account/model/reasoning | Direct provider → advanced provider options |
| Change the folder | Folder control or `/workspace <absolute path>` |
| Compact | Context popover or `/compact` |
| Close a dialog | Escape |

Projects organize conversations; moving a conversation between projects does not change its working folder. Folder changes are blocked while its turn is active. Earlier messages retain their original workspace association.

## Browser, images and inspector

Request web research or page interaction normally: Moli is automatically connected. Read-only profiles cannot submit page mutations. Captured images appear in the transcript; click to enlarge. The project inspector adds local previews, changes/checks, visual review, image/data inspection and layout constraints. Optional Godot actions require a compatible configured editor/addon.

## Background work and recovery

By default X hides Lumen to the notification area and work continues. Use the tray menu to reopen or Quit. After a crash or computer restart, unfinished work is marked interrupted and resumes only when requested. Latest partial text may not have reached its next persistence checkpoint.

## Troubleshooting

- **CLI missing:** install its official distribution, confirm PATH and restart Lumen.
- **Login unavailable:** sign in for that exact account slot. Existing terminal credentials are not automatically copied into Lumen.
- **Model/reasoning rejected:** refresh the catalog and select a supported combination or provider default.
- **Quota reached:** wait, choose another eligible provider or use the configured rate-limit fallback. Missing usage data is not proof of free quota.
- **Tool denied:** check the actual permission mode and provider policy. Approvals cannot grant an unsupported native operation.
- **Ollama offline:** start its local service and install a conversational model. Remote Ollama hosts/cloud aliases are rejected.
- **Browser blocked:** Moli may encounter CAPTCHA, authentication or unsupported rendering. The error is explicit; it does not silently switch browsers.
- **Stuck work:** Stop, inspect activity and retry with the native CLI if compatibility/login needs diagnosis.

Deleting conversations/accounts removes their local Lumen data after confirmation; remote revocation and CLI-owned history are separate. [Security](../SECURITY.md) · [Known validation limits](validation.md).
