# Security and privacy

Lumen is an alpha, AI-written local application with no independent security audit. Provider tools, project instructions and OS permissions are all part of its trust boundary.

## Permissions

- The profiles (Read only, File edits, Autopilot, Approve each action) map onto native provider controls and Lumen's own tool loops. **They are not an OS sandbox.**
- Autopilot grants every available tool under the current OS user. It does not bypass UAC, OS ACLs or provider policy.
- Reviewers are read-only. Questions and past approvals never raise the profile.
- Risky commands always ask in Lumen's own loops (APIs and Ollama). Native CLIs in Autopilot are logged, not pre-blocked. See [autonomy](docs/product.md#recoverable-autonomy).

## Credentials

- Lumen never copies tokens, implements provider OAuth or runs an inference gateway. Each CLI logs in through its own flow into an isolated slot directory, which may contain plaintext credentials owned by that provider.
- Inherited API-key variables are removed from the CLIs Lumen launches.
- Official API keys live only in the OS keychain, never in environment variables or SQLite. Requests go to fixed HTTPS endpoints with no proxies or redirects. Spend is bounded by durable reservations taken before each request.
- Provider binaries come from `PATH`, so install them from official sources.

## Browser, Ollama and plugins

- Moli archives are pinned and verified by SHA-256. The broker is loopback-only, uses a per-invocation capability and enforces the profile itself. Page content is untrusted data.
- Ollama is loopback-only. Remote hosts, proxies, redirects and cloud models are rejected.
- Plugin MCP servers run with a cleared environment and only the secrets they declare, but they are not OS-sandboxed. Install plugins only from authors you trust.
- Full-access shells and external MCP servers keep their own network access. Lumen is not a firewall.

## Data

- `lumen.db`, the event journal, account directories, Moli profiles and captures in `.lumen/screenshots/` are **not encrypted by Lumen**. Protect your OS account and use disk encryption where you need it.
- There are no analytics and no Lumen cloud.
- Deleting a conversation or an account is local. Revoking access on the provider side is separate.
- Never upload account directories, private transcripts or unredacted logs.

## Local service

The IPC transport authenticates the OS peer: SID checks on a named pipe on Windows, peer UID checks on a private socket on Unix. It also checks protocol and build compatibility. There is no TCP listener. A malicious process running as the same user is outside this boundary. [Details](docs/architecture.md#local-security-boundary).

## Reporting

Use [private vulnerability reporting](https://github.com/Andrea17982/Lumen/security/advisories/new). Include your OS, the app and CLI versions, and a minimal redacted reproduction. There is no response SLA and no bounty.
