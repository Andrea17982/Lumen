# Projects, models and permissions

## Conversation location

A project is a named container, independent of its conversation folders. Moving, dragging or regrouping a conversation preserves its actual working directory. The folder control exposes the path; `/workspace <absolute path>` applies an explicit change before planning or tools start. References to another project, quoted instructions and ambiguous prose do not silently switch roots. A running/queued turn must finish or stop before its folder changes.

The session stores its project association, folder and permission profile. Messages retain workspace metadata for links and images. Spill files and browser captures use the appropriate conversation project; worker screenshots survive disposal of a worktree.

## Accounts and model catalogs

Rename changes only a label, keeping the account UUID, isolated configuration directory and login. Direct provider options support all connected providers. Catalogs are cached per account for five minutes and can be refreshed. OpenCode project catalogs are queried in the selected workspace instead of reusing a global account catalog.

| Provider | Catalog / execution |
| --- | --- |
| Claude | Native initialize model metadata; bidirectional stream-json turns |
| Codex | app-server `model/list`, including pagination; app-server turns |
| Antigravity | Official `agy models` metadata and print runner |
| Ollama | Official loopback `/api/tags` and `/api/show`; local `/api/chat` tool loop |
| OpenCode | Project-scoped `models --standalone`, official ACP and session-policy API; native continuation, all four permission profiles, explicit selection |
| OpenAI API | Official `/v1/models` and Responses with Lumen host tools; priced-model allowlist |
| Anthropic API | Official `/v1/models` and Messages with Lumen host tools; priced-model allowlist |
| Google API | Official `v1beta/models` and generateContent with Lumen host tools; priced-model allowlist |
| Cursor CLI | Official `--list-models`, `--print --output-format stream-json`, stdin and `--resume`; explicit Autopilot only |

Only advertised model/reasoning combinations are accepted. Missing metadata is a recoverable error rather than a fabricated list. The explicit choice is preserved; model availability still depends on account/organization policy. Ollama lists local conversational models, with the pinned/largest one marked default, and rejects cloud-routed metadata.

## Permission profiles

| Profile | Intended behavior |
| --- | --- |
| Read only | Initial profile. Provider read-only controls; Moli retrieval/navigation/extraction/capture, without page mutation. |
| File edits | Workspace editing through supported native controls. Moli page mutations remain denied. Antigravity's headless profile also allows shell commands; the UI explains this difference. |
| Autopilot | Full available provider tools under the current OS account, without CLI approval prompts or repeated Lumen planning approvals. Moli actions proceed directly. |
| Approve each action | Native callbacks for Claude/Codex/OpenCode and one-shot host decisions for the official APIs; supported requests wait for an immutable scoped decision. Ollama browser mutations use the shared browser decision path. Google print runners do not have equivalent general bidirectional approval support. |

OpenCode verifies native per-session policies before prompting: Read only/File edits deny shell, external directories, custom subagents and Code Mode; Ask bridges one-shot ACP decisions. Moli/graph and supported shared MCP are injected with invocation scope. Cursor rejects restricted/Ask profiles and retains Autopilot-only native tools. Both use the existing native profile without isolated logins or automatic entry. Native profile/team policies still apply. The three [API adapters](api-access.md) enforce all four profiles through the common Lumen host tools.

Read-only review/analysis workers stay restricted. Changing the current profile applies to subsequent work and native resumed turns; it does not rewrite an action already in flight.

Autopilot does not override OS ACLs/UAC, provider policy or missing capabilities. Read-only mode is not an additional OS filesystem jail. Full-access shells and external MCP tools are not a hard network sandbox. Model questions, clarified requirements and historical approvals cannot elevate the actual permission profile.

## Native questions and child agents

Questions can offer radio/checkbox choices, free text or secret input. Closed connections reject stale responses; secret answers are excluded from durable decision text. Claude/Codex expose native child-agent progress/output separately from the answer. Native child sessions are not the same as Orchestra workers.

[Feature guide](features.md) · [Browser permissions](moli.md) · [Security](../SECURITY.md) · [Verification matrix](validation.md)
