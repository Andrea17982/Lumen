# Accounts and official authentication

## What Lumen does

You install the official, unmodified `claude` / `codex` CLI. An account slot gives it a separate application-owned configuration directory. Choosing Sign in opens the CLI's official login command in a visible Windows PowerShell terminal with that directory set. The CLI owns its browser, callbacks, tokens and refresh logic. Lumen neither implements OAuth endpoints nor reads token files.

| Provider | Environment | Official login | Connection check |
| --- | --- | --- | --- |
| Claude Code | `CLAUDE_CONFIG_DIR=<app-data>/accounts/<id>` | `claude auth login` | `claude auth status` |
| Codex | `CODEX_HOME=<app-data>/accounts/<id>` | `codex login` | `codex login status` |

There is no application-set account count cap. This does not increase a provider's allowance, guarantee every authentication store is independent, or authorize credential sharing. One slot per identity is recommended; duplicate slots for the same identity still share provider-side limits. Auto never cycles accounts after a quota error.

## Provider rules matter

Anthropic distinguishes hosting an unmodified Claude Code binary for users who authenticate for themselves from offering Claude.ai login or routing subscription credentials as a third-party inference service. Lumen takes the former local-CLI approach: it does not collect credentials, implement a Claude login service, modify the binary, resell usage or send direct requests using extracted tokens. This implementation choice is not a legal certification or a blanket permission for every deployment. Review [Anthropic's current legal and credential-use rules](https://code.claude.com/docs/en/legal-and-compliance), including the commercial-terms requirement for offering Claude Code in a product. Obtain provider authorization where your distribution or use requires it. Never adapt Lumen into a shared subscription gateway.

Codex CLI supports official ChatGPT sign-in, and its exec mode reuses CLI-managed authentication. Lumen delegates to these features; see [OpenAI authentication](https://developers.openai.com/codex/auth) and [noninteractive mode](https://developers.openai.com/codex/noninteractive). Availability depends on the user's plan and organization policy. No credentials are seeded in public CI.

## Isolation and storage

- Names are labels only. IDs are generated UUIDs; user names never become paths.
- Common inherited API credentials are removed from provider children. Existing home-directory logins are not imported.
- Codex account creation writes `cli_auth_credentials_store = "file"` in that isolated home. The CLI may store plaintext tokens there; protect the directory as you would a password.
- `CLAUDE_CONFIG_DIR` isolates Claude settings/history as documented in [Claude settings](https://code.claude.com/docs/en/settings). OS keychain behavior may vary. Distinct directories alone do not prove independent keychain identities; acceptance-test with two accounts on your OS.
- Account directories use the current user's standard application-data permissions; Lumen does not create an additional encrypted credential vault or hardened ACL.
- Removing an account deletes that Lumen-owned directory and metadata after user confirmation, while no jobs are active. It does not revoke remote authorizations or remove a possible shared OS-keychain item.
- Log out/revoke access through the official provider tools when necessary. Do not manually share token files.

## Linux manual login

On Linux the Sign in action reports that terminal launching is unsupported. Resolve the app-data location from Tauri's platform convention (typically `$XDG_DATA_HOME/io.github.andrea17982.lumen` or `~/.local/share/io.github.andrea17982.lumen`) and the account UUID from its folder, then run in your terminal:

```sh
CLAUDE_CONFIG_DIR='/path/to/lumen/accounts/account-uuid' claude auth login
CODEX_HOME='/path/to/lumen/accounts/account-uuid' codex login
```

Keep the quotes. Return to Lumen and check the connection. Linux is a secondary target, with no claim of real-device acceptance testing in this release.
