# Accounts and official authentication — 0.7

Use your own authorized provider account. Lumen delegates subscription authentication to the installed official CLI, without implementing OAuth endpoints, copying token contents or sending extracted credentials to provider APIs.

| Provider | Configuration / login | Connection |
| --- | --- | --- |
| Claude Code | Per-account `CLAUDE_CONFIG_DIR`; `claude auth login` | Official auth status and initialized native metadata |
| Codex | Per-account `CODEX_HOME`; `codex login` | Official login status and app-server metadata |
| Gemini CLI | Per-account `GEMINI_CLI_HOME`; official CLI login | Installed CLI metadata |
| Antigravity | Separate `HOME`/`USERPROFILE`; official `agy` login | `agy models` |
| Ollama | Your installed local models; no login | Official loopback `/api/tags` |

Add and name slots in Settings → Accounts, choose Sign in, finish the provider's native terminal/browser flow and check the connection. Renaming preserves the account UUID and configuration directory. Restart after installing a CLI or changing PATH. Native login launch uses platform terminal support; manual environment-based login remains an option if a terminal is unavailable.

## Model and provider access

All five providers can be selected directly. Catalog discovery is per account and does not itself establish entitlement to every returned model. Provider plans, organization policies, CLI versions and actual quotas still apply. Auto's optional balancing/fallback is described in [Auto policy](auto.md); duplicate account slots do not create additional allowance.

Ollama conversations, discovery and helpers use its official local API. `OLLAMA_HOST` can select another loopback HTTP port; remote endpoints, proxy/redirect routing and cloud-model metadata are rejected. Lumen never pulls a missing model automatically. Install models through Ollama yourself. The browser loop does not add Claude/Codex filesystem or child-agent tools to Ollama.

## Local directories and credentials

Account metadata is stored in `lumen.db`; native configuration/history lives under `<app-data>/accounts/<UUID>`. Codex uses a file-backed store in its account home. Common inherited provider API-key/token environment variables are removed before subscription CLI execution to avoid silently selecting another identity/billing path.

Native OS keychains or provider-specific stores may still be shared across versions. Separate directories are not proof of independent identities; test distinct accounts on the relevant OS before relying on it. No extra encrypted credential vault is implemented. Removing an account deletes its Lumen-owned directory after confirmation while jobs are stopped; remote revocation and shared keychain cleanup remain with the provider.

For manual login, use the actual isolated slot path, for example:

```sh
CLAUDE_CONFIG_DIR='/path/to/lumen/accounts/account-uuid' claude auth login
CODEX_HOME='/path/to/lumen/accounts/account-uuid' codex login
```

Do not paste tokens into conversations or upload account directories in bug reports. Provider terms and distribution requirements apply independently; Lumen's native-CLI implementation is not legal certification. Consult the providers' official terms and authentication documentation for your deployment.

[Permissions](workspace-models-permissions.md) · [Security](../SECURITY.md) · [Observed provider acceptance](validation.md)
