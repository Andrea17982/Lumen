# Security and privacy

Lumen is an alpha, completely AI Written application. It has not received an independent security audit. Use a trusted project folder, keep provider tools current, and inspect edits before committing or deploying them.

## Trust boundaries

- The webview has a restrictive CSP, no remote script loading and no shell/filesystem plugin. Rust exposes specific application commands rather than a generic process launcher.
- Model Markdown does not render raw HTML, fetch remote images or open arbitrary links. Project-controlled CLI settings, instruction files and provider behavior are still relevant trust boundaries.
- The provider binaries are installed by the user and discovered on PATH. A malicious PATH entry can substitute a binary. Prefer official installers and a trusted PATH.
- Prompts are stdin, not shell command strings. Names never enter account paths. Models are validated before process creation.
- Read-only and workspace-write are provider controls. Lumen does not provide an independent OS sandbox around the CLI. Claude is limited to Read/Glob/Grep in read-only turns and also Edit/Write in write-enabled turns, with `--restricted` confining file tools to the working directory. In these two profiles Claude receives no Bash tool from Lumen. Codex uses its read-only or workspace-write sandbox with noninteractive approvals disabled. **Autopilot is a separate, explicitly selected full-access profile:** Claude receives `--dangerously-skip-permissions --tools default` without restricted/strict-MCP overrides; Codex receives `--dangerously-bypass-approvals-and-sandbox` without a sandbox flag. It allows network, shell, filesystem and configured automation tools under the current OS account.
- Autopilot deliberately allows the provider to act without CLI approval prompts, including shell commands and file changes. It does not override UAC, OS ACLs, managed provider policies or missing computer-use tools. Read-only reviewers remain restricted. Provider versions can change permission behavior. Auto cross-provider stages share the relevant task context with both configured providers.

## Data

`lumen.db` contains conversation text, workspace paths, titles and exposed activity. It is not encrypted by Lumen. Provider-owned directories may contain tokens and histories. Codex file-backed credentials are deliberately local to each account directory; secure the application-data directory and OS account. Lumen does not read credential contents or ship tokens in builds.

UI theme/default-folder preferences use webview local storage. There is no app analytics service, telemetry SDK, Lumen backend or inference proxy. Official CLIs communicate with their providers and may have their own telemetry and retention policies. PayPal opens externally only after the user chooses support; payment credentials never enter Lumen.

Conversation deletion is permanent locally, without deleting CLI history or remote provider records. Account deletion removes Lumen-owned files and metadata after confirmation; remote revocation is separate. Removing an app or uninstalling may leave application data. Back up only what you understand and never post an entire account directory in a bug report.

## Process lifecycle

Cancellation signals queued and active stages, kills the Windows CLI process tree and reaps it. The application requests cancellation on normal exit. OS crashes, power loss and forced termination may prevent cleanup. After restart, waiting/running persisted sessions become interrupted. Check for orphan provider processes if the application is force-killed.

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/Andrea17982/Lumen/security/advisories/new) if enabled. If unavailable, open an issue asking for a private contact without posting exploit details, credentials or private transcripts. Include OS, app version and CLI versions. There is no promised response SLA or paid bug bounty.
