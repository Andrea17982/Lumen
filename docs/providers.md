# Providers, permissions and integrations

Lumen uses each provider only through its official interface. Subscription logins stay in each official CLI, and API keys stay in the OS keychain. Lumen never extracts tokens, implements provider OAuth or runs an inference proxy.

## Accounts

| Provider | Isolation and login | Catalog and execution |
| --- | --- | --- |
| Claude Code | Per-slot `CLAUDE_CONFIG_DIR`; `claude auth login` | Native initialize metadata; bidirectional stream-json |
| Codex | Per-slot `CODEX_HOME`; `codex login` | app-server `model/list` and turns |
| Antigravity | Separate `HOME`/`USERPROFILE`; `agy` login | `agy models`; print runner |
| OpenCode | One existing native profile; `opencode auth login` | Project-scoped `models --standalone`; official ACP |
| Cursor CLI | One existing native profile; `cursor-agent login` | `--list-models`; headless stream-json; Autopilot only |
| Factory Droid | One existing native profile; `/login` in `droid` | No model listing: the profile default plus the default model and favorites from Droid settings; `droid exec` stream-json; Read only, File edits, Autopilot |
| Ollama | No login | Loopback `/api/tags`, `/api/show`, `/api/chat` tool loop |
| OpenAI / Anthropic / Google API | Key in the OS keychain per slot | Official HTTPS catalog; Responses / Messages / generateContent |

- Slot directories live under `<app-data>/accounts/<UUID>`. Renaming a slot changes only its label.
- Lumen strips inherited API-key variables (including `CURSOR_API_KEY` and `FACTORY_API_KEY`) before launching a CLI, so billing identity never changes silently.
- Separate directories do not prove separate identities: OS keychains can be shared.
- Catalogs are cached for 5 minutes. Lumen accepts only model and reasoning combinations the provider advertises, and never fabricates a list.
- OpenCode, Cursor and Droid are always selected explicitly. They are never Auto fallbacks or Orchestra workers.
- Ollama runs on loopback only. Lumen rejects remote hosts, proxies, redirects and cloud models, and never pulls a model automatically.

Manual login into a slot:

```sh
CLAUDE_CONFIG_DIR='/path/to/lumen/accounts/<uuid>' claude auth login
CODEX_HOME='/path/to/lumen/accounts/<uuid>' codex login
```

## Permission profiles

| Profile | Behavior |
| --- | --- |
| Read only | Initial profile. Read-only provider controls; Moli can read and capture but not mutate pages. |
| File edits | Workspace edits through native controls. Antigravity's headless profile also allows shell commands. |
| Autopilot | All available tools under the OS user, without approval prompts. Not administrator, not an OS sandbox. |
| Approve each action | Native callbacks (Claude, Codex, OpenCode) or one-shot host decisions (APIs). Google print runners have no equivalent. |

- Reviewers and analysis workers are always read-only.
- A profile change applies from the next action.
- Questions, clarified requirements and past approvals never raise the profile.
- Native provider and organization policies still apply.

## Adapter capabilities

The versioned Rust contract (`capability_catalog.rs`) declares what each of the nine adapters supports. "Not available" and "not implemented" are never reported as done.

| Adapter | Session | Profiles | Shared tools | Usage / cost |
| --- | --- | --- | --- | --- |
| Claude Code | Native continuation | All four | Native MCP + filtered owner gateway | Input when emitted; no cost |
| Codex | Native continuation | All four | Native MCP + filtered owner gateway | Input when emitted; no cost |
| Antigravity | Context continuation | No Approve each | Native namespaces or gateway | Input when emitted; no cost |
| OpenCode | ACP load/continuation | All four, verified session policy | Moli/graph + shared MCP via ACP | Unavailable |
| Cursor CLI | Headless continuation | Autopilot only | Native profile only | Unavailable |
| Factory Droid | Headless continuation | Read only, File edits, Autopilot (`--auto`) | Native profile only | Unavailable |
| Ollama | Context continuation | No Approve each | Filtered host tool loop | Context estimate |
| OpenAI / Anthropic / Google API | Lumen context | All four | Common host tools | Reported usage + estimate, reserved budget |

## Shared integrations

Lumen discovers MCP servers, skills and Claude plugin components in standard profiles, Lumen slots and the current project, and adapts them across providers. This works for a documented subset, **not every proprietary extension**.

- **Namespaces** are stable per source. Identically named servers never overwrite each other, and one project's components never leak into another.
- **Skills** are listed to every provider and read on demand.
- **stdio MCP** goes through a local relay. **Streamable HTTP MCP** uses the official Rust SDK. Headers are never forwarded through redirects.
- **Codex account gateway.** Through direct RPC, another provider can call tools that belong to a Codex account, without starting a GPT turn.
- **Consent.** MCP elicitation is answered in Lumen and expires on Stop. Answers are never saved as chat requirements.
- **Restricted profiles.** Read only and File edits see only the filtered gateway. A destructive tool never enters a read-only catalog.
- Auto never transfers Claude connector authorizations to another provider.

Not supported:

- Claude.ai cloud connectors.
- Cross-provider hooks, mods, commands and native agents. Lumen detects them but does not share them.
- Legacy SSE and WebSocket MCP.
- Translating credentials between owners.

## Official APIs and budget

1. Add an API slot and save the key. It goes to the OS keychain. There is no plaintext fallback, and a locked keychain blocks API use.
2. Enter an exact model ID with its highest input and output USD rates per million tokens. One slot covers one priced model.
3. Set a cumulative USD budget and an output cap of 256–32768 tokens.
4. **Allow Auto to use this API** is a separate setting and is off by default.

`api.sqlite` reserves the maximum estimated cost in a transaction **before every HTTP request**, including each tool-loop step. Reported usage then replaces the reservation. Stops, crashes and uncertain responses keep the reservation. The displayed cost is an estimate, not an invoice.

Requests are capped at 8 MB and 32 model steps per turn. Endpoints are fixed HTTPS, with no proxies or redirects.

The API adapters do not support native session resume, streaming (responses are buffered JSON), native image input or native skill, subagent or hook runtimes. They are not Orchestra workers.

## Code Mode

In **Settings → General**, Code Mode is one switch: on means `auto`, off means `off`. Underneath it is `codeMode` in the daemon's `settings.json`:

- **`auto`** (the default). The turn starts with classic tool calls and switches to `lumen_code` scripts after 5 tool rounds or a result over 24 KiB.
  - On Claude Code, Lumen proxies Moli and MCP. Results over 8 KiB become context documents, and Claude sees only their first 2 KiB.
  - Codex and OpenCode keep their direct tools.
  - On Ollama, only measured families switch (currently gpt-oss). `ollamaCodeModels` forces Code Mode for exact model IDs.
- **`on`**: scripts from the first call, for every provider. MCP tools are then reachable only from scripts.
- **`off`**: classic tool calls only.

Script limits: 200 calls, 8 in flight, 120 s and a 256 MiB heap. Each call follows the normal permission path.

Context API: `context.list/get/slice/grep/chunks`, with a 32 MiB store per turn.

`llm(prompt, {effort, maxTokens})` sends a subquery to a tool-less model. It prefers a local model, otherwise an opted-in API account, whose budget is reserved. `maxTokens` ranges from 16 to 4096 (default 512).

| Effort | Subqueries | Depth | In flight |
| --- | ---: | ---: | ---: |
| low | 0 | 0 | 0 |
| medium | 8 | 1 | 4 |
| high | 32 | 1 | 8 |
| max | 128 | 2 | 8 |

The CLI MCP server runs as `lumen-daemon --lumen-code-mcp`. Measurements: [Validation](validation.md).
