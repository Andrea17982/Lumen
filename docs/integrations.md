# Shared integrations

Lumen discovers existing MCP servers, skills and plugin components and makes portable components available through supported adapter paths. OpenCode bridges this layer through official ACP; Cursor retains its native tools without shared injection. The three official API adapters use the same Lumen host tool layer. Identity, consent and the original configuration stay with the owning profile. This is a documented subset, **not universal compatibility** with every proprietary extension; declared limits remain visible in the settings matrix.

## What is supported

### Provider capabilities

**Settings → Advanced → Tools → Provider capabilities** reads the versioned Rust contract for all nine adapters. Capabilities describe Lumen’s integration, separately from installation, login, entitlement and model quality. The matrix includes session resume, approvals, native skills, shared tools, usage/cost, cancellation, permission profiles, streaming, image input, subagents, extensions, model selection, fallback and normalized events. Unknown events are retained conservatively; missing tokens/prices are never fabricated.

| Adapter | Native session | Permission profiles | Shared tools | Usage / monetary cost |
| --- | --- | --- | --- | --- |
| Claude Code | Native continuation | All four | Native MCP and filtered owner gateway | Input when emitted; monetary cost unavailable |
| Codex | Native continuation | All four | Native MCP and filtered owner gateway | Input when emitted; monetary cost unavailable |
| Antigravity | Context continuation | Read only / File edits / Autopilot | Native shared namespaces or filtered owner gateway | Input when emitted; monetary cost unavailable |
| OpenCode | Official ACP load and continuation | All four, verified native session policies | Moli/graph plus supported shared MCP through ACP | Input/cost unavailable in the ACP bridge |
| Cursor CLI | Native headless continuation | Autopilot only | Existing native profile only | Input/cost unavailable |
| Ollama | Context continuation | Read only / File edits / Autopilot | Filtered host tool loop; model dependent | Context estimate; monetary cost unavailable |
| OpenAI API | Conversation/task context | All four | Common Lumen host tools | Reported usage + user-rate estimate / reserved budget |
| Anthropic API | Conversation/task context | All four | Common Lumen host tools | Reported usage + user-rate estimate / reserved budget |
| Google API | Conversation/task context | All four | Common Lumen host tools | Reported usage + user-rate estimate / reserved budget |

OpenCode 2.2.7 uses its official session API to create/update and read back permissions **before any prompt**, then official ACP over stdio. Session IDs are checked against the conversation workspace. Read only/File edits deny shell, external directories, Code Mode and custom subagents; File edits permits internal edits. Ask exposes direct tools and bridges Allow once/Deny; there are no permanent permission grants. Native hard policies remain in effect. Loading old history does not duplicate it into the current reply. Model selection is confirmed through ACP configuration; rejection never silently substitutes a model. Project-specific catalogs are discovered in the selected workspace.

OpenCode and Cursor use one existing native profile each, stay explicitly selected, and are not automatic fallback destinations or Orchestra workers. Cursor’s headless bridge requires a successful terminal result as well as process success. Both have owned-tree Stop. OpenCode’s proprietary interactive callbacks beyond ACP permissions are declined explicitly; native reasoning choices are not guessed from a catalog that does not expose them.

The [official APIs](api-access.md) use OS-keychain keys and independent billing identity. Explicit Auto opt-in plus a mandatory model budget is required at each paid request. They currently buffer JSON responses, support text inputs/common tools, and do not implement native session/image/skill/hook/subagent runtimes or arbitrary reasoning controls. API accounts are not Orchestra workers in this milestone. This is a declared adapter boundary, not universal native parity.

Official contracts: [OpenCode API](https://opencode.ai/docs/api), [OpenCode permissions](https://opencode.ai/docs/permissions/), [OpenCode hard policies](https://opencode.ai/docs/policies/), [ACP sessions](https://agentclientprotocol.com/protocol/v1/session-setup), [Cursor headless](https://prod.cursor.com/docs/cli/headless), [Cursor events](https://prod.cursor.com/docs/cli/reference/output-format), [Cursor permissions](https://prod.cursor.com/docs/cli/reference/permissions).

### Shared integration behavior

| Area | Behavior |
| --- | --- |
| Discovery | Standard Claude, Codex and Antigravity profiles, Lumen account directories and the current project. JSON/TOML configuration, local skills and the MCP/skill components of installed Claude plugins (user, and correctly scoped project/local) keep their original files and resources. Nothing is installed and no login store is copied. |
| Namespaces | Stable per-source namespaces: identically named services from different accounts never overwrite each other, project A components never enter project B, and disabled entries stay disabled. Unconverted policies, authentication or transports are reported as needing an adapter rather than silently simplified. |
| Skills | Listed in every provider's context with original path and description; content is read on demand. |
| stdio MCP | A local relay reads the source at launch and preserves MCP frames in both directions; process arguments carry references, not server secrets. |
| Streamable HTTP MCP | Official Rust MCP SDK client with TLS, original headers, sessions and SSE responses. Expired sessions do not replay tool calls; headers are connection-scoped and are not forwarded through redirects or proxies. |
| Codex account gateway | `initialize`, an ephemeral thread, `mcpServerStatus/list`, `mcpServer/tool/call`, `skills/list` and resource reads through the owning ChatGPT/Codex account. It never sends `turn/start` or a GPT prompt. |
| Ollama | Native tool calling and constrained JSON actions consume real results from shared stdio/HTTP MCP, scoped skill resources, MCP resources and prompt templates (as data). Tool-list changes refresh the catalog without duplicating history. |
| Consent | MCP elicitation (form/URL) is handled as connector consent in Lumen's own UI, typed and private, and expires on withdrawal, Stop and connection close. Responses are never saved as shared chat requirements. Claude MCP tool approvals are reviewed in Ask first. |
| Permissions | Shared namespaces passed directly to native CLIs require Ask first or Autopilot. Read only/File edits expose only the filtered owning-account gateway; Ollama filters per tool. A declared destructive tool never enters a read-only catalog. |
| Lifecycle | Per-invocation Claude configurations are removed at turn end. Google clients that read MCP settings from the account home serialize turns on the same account (cancellable). Relays and gateways run under process-tree supervision (Windows Job Object, Unix process group). |

**Settings → Advanced → Tools** lists detected integrations, their origin, configuration state and missing adapters. **Share** changes only Lumen's preference; the source stays untouched. "Configured" means a declaration was found, not that login or a remote connection succeeded.

## Compatibility audit and explicit limits

| Area | Missing piece |
| --- | --- |
| Claude.ai cloud connectors | Not portable: no verified public execution API equivalent to the Codex gateway; some connectors are bound to Anthropic's registered OAuth redirect. Copying credentials is not an adapter. |
| Hooks, mods, commands, native agents | Native profiles retain their own supported runtime. Cross-provider execution is not implemented: events/semantics are proprietary and copying files would change behavior. Hooks are detected and reported, not shared. |
| Legacy SSE / WebSocket | No legacy-SSE transport adapter; Streamable HTTP with SSE responses is a different, supported protocol. No common standard WebSocket MCP bridge is implemented. Discovery marks these declarations as needing an adapter and never drops their transport. |
| OAuth/helpers and policy formats | No automatic cross-owner credential/helper translation. Native owners must authenticate; unresolved variables, unknown policies or auth options remain needs-adapter. No secrets are copied. |
| Restricted native modes | A per-tool filter in native clients, so portable services can be exposed in Read only/File edits beyond the owning-account gateway. |
| MCP extensions | Proprietary tasks/notifications, prompts/APIs not exposed by the account gateway, and extension UIs. |
| Real-account matrix | GitHub, Gmail, Drive and other services on real accounts; macOS/Linux desktops; expired authentication, revocation and concurrency. Fixtures do not certify these. |

Auto can choose another provider, but it never transfers Claude connector authorizations to GPT. Local Git/`gh` authentication is separate from a GitHub connector.

## Verification

`node tests/manual/adapter-corpus.mjs` runs the common offline acceptance scenarios serially, preserving each native protocol’s fixtures. It covers core agents and all three APIs through Windows IPC/storage/owned processes. `node tests/manual/native-opencode-installed.mjs` separately exercises the installed OpenCode 2.2.7 runtime with a temporary project, real ACP/session-policy API and a deterministic loopback model. Project policy denies other providers: this tests native enforcement without paid inference. See [Validation](validation.md).


Offline Windows harnesses `node tests/manual/native-shared-integrations.mjs` and `node tests/manual/native-connectors.mjs` cover discovery, the stdio relay and process-tree cleanup, the Codex gateway without model turns, skill resources, restricted catalogs, consent forms/URLs, Stop expiry and a Claude → ChatGPT-account tool call. `node tests/manual/native-ollama-shared.mjs` covers the local tool loops. `node tests/manual/native-installed-mcp.mjs` is an opt-in interoperability probe against the installed Codex CLI with a temporary profile and no inference. Run native harnesses and Playwright serially (shared port 1420). See [Development](development.md).

Primary sources: [Codex app-server](https://learn.chatgpt.com/docs/app-server), [Claude MCP and connectors](https://code.claude.com/docs/en/mcp), [Claude Agent SDK](https://code.claude.com/docs/en/agent-sdk/typescript), [Antigravity MCP](https://antigravity.google/docs/mcp), [Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk).
