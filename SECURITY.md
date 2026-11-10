# Security and privacy — 0.7

Lumen is an alpha, AI-written local desktop application without an independent security audit. Provider tools, project instructions and OS permissions remain part of its trust boundary.

## Permissions and external tools

Read only, File edits, Autopilot and Approve each action map to supported native controls; they are not an additional common OS jail. Restricted Claude modes expose explicit read/check command allowances rather than arbitrary shell access. Codex uses its native sandbox/approval protocol. Antigravity File edits permits commands under its documented headless rules, with a UI notice. Google print runners do not provide the same general approval callback interface as Claude/Codex.

Autopilot authorizes available native tools, shell/network/filesystem actions and configured extensions under the current OS account without repeated Lumen plan approvals. It does not override UAC, OS ACLs or provider policy. Review/analysis workers stay read-only. Questions and historic approvals cannot elevate the actual profile. Unsupported native approval callbacks remain unavailable, not automatically accepted.

Provider binaries come from PATH, so use trusted official installations. Subscription authentication remains inside their own CLI flows/directories. Lumen does not copy token contents, implement provider OAuth endpoints or turn subscriptions into a shared inference gateway.

## Browser and Ollama

Bundled Moli archives are pinned and SHA-256 verified at preparation time, with upstream notices retained. The browser broker is loopback-only, uses an invocation-specific capability and validates tool arguments, access, output/concurrency/deadlines and cancellation. Page content is untrusted data. Read only/File edits reject page mutations; Autopilot proceeds; Ask-first browser mutations require a scoped one-action decision.

Moli routing disables supported native browsing alternatives. Full-access shells and independently configured MCP servers remain available; this is not an OS network firewall. Web browsing contacts external websites/search engines, whose own behavior and policies apply.

Ollama inference/discovery/helpers use its official API on this computer. Non-loopback hosts, redirects/proxies and cloud-model metadata are rejected. No inference key or automatic model pull is used. An arbitrary local service running as the user remains within that user's trust boundary.

## Data and interface

`lumen.db` stores conversations, project paths, exposed activity and task decisions. Account directories contain provider-owned settings/history and may contain plaintext credentials. Moli keeps isolated local browser profile data. Captures are saved under the conversation project's `.lumen/screenshots/`; that directory ignores itself in Git. None of these stores is encrypted by Lumen. Protect the OS account/app-data directory and use disk encryption where required.

The webview has a restrictive CSP, no remote script loading and specific Rust commands rather than a generic shell/filesystem plugin. Markdown excludes raw HTML. Remote response images load on user action, local image/link resolution stays within the supplied project root, and UI decisions do not carry executable approval inputs. Secret question answers are excluded from public durable decision text.

There is no Lumen analytics service or general inference proxy/server. Native provider tools have their own network/retention policies. PayPal opens externally only on user action; Lumen receives no payment credentials.

Conversation/account deletion is local and confirmed; remote revocation and CLI history are separate. Uninstalling may leave app data. Never upload entire account directories, private transcripts or unredacted logs.

## Process lifecycle

Closing to the notification area preserves active work. Stop/normal Quit request cancellation and owned process cleanup. Crashes, power loss and forced termination can prevent cleanup. Restart marks unfinished work interrupted and requires explicit Resume; recovery verifies old process identity instead of blindly acting on a reused PID.

## Reporting

Use [private vulnerability reporting](https://github.com/Andrea17982/Lumen/security/advisories/new) when available; otherwise request a private contact in an issue without exposing exploit details or secrets. Include the OS, app/CLI versions and a minimal redacted reproduction. There is no promised response SLA or paid bounty.

[Permission guide](docs/workspace-models-permissions.md) · [Validation](docs/validation.md)
