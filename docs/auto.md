# Auto execution policy · 0.4

Auto routes locally without a classifier request. It selects an actual model ID from the selected account's official CLI catalog, including for reviewers and parallel workers. Unavailable model names are never manufactured. These are explainable heuristics calibrated against the user's examples.

| Request | Preferred model class | Default effort |
| --- | --- | --- |
| Test, short text, translation/email | Haiku / Luna; Terra if no smaller model is available | Low |
| CSS fix, coding, ordinary debugging | Sonnet / Terra | Low, or medium for longer requests |
| Intermittent concurrency bug | Sonnet / Terra | Medium |
| Database migration, architecture, authentication/security review | Opus / Sol | Low, or medium for complexity/production signals |

Version numbers come from the account catalog. A long email can raise effort without automatically requiring a flagship. Unrecognized models fall into the middle class; catalog defaults break ties. Routing can misinterpret wording. Follow-ups are classified from the latest request; use manual controls when continuity requires a particular model.

The three policies persist locally:

- **Budget:** one effort step lower, clamped at low; suppresses extra review/parallel stages.
- **Default:** calibrated effort and independent review for demanding architecture/security/migration work or explicit validator requests.
- **Quality:** one effort step higher; adds an independent reviewer for coding and advanced tasks when another provider is configured. Simple messages stay single-stage.

Comparisons with both providers and Default/Quality gather two independent perspectives, then synthesize. Manual provider/model/effort selection uses a single stage, validated against official metadata. Each stage resolves effort to the nearest supported level or provider default and records the effective model/effort in activity.

## Orchestra

With Settings → Teamwork → **Orchestra** on, Auto hands complex requests to an orchestrator instead of a single writer. "Complex" uses the same local profile: the Opus/Sol class (architecture, security, migration), or the Sonnet/Terra class when the request is demanding or longer than about 600 characters. The orchestrator's model class is the task's class with a Sonnet/Terra floor under Budget and Default, and the strongest class under Quality. Each subtask gets a class from its difficulty (trivial → Haiku/Luna, normal → Sonnet/Terra, complex → Opus/Sol) and an effort of low, or medium for complex work, moved one step by Budget or Quality. Subtasks alternate between Claude and ChatGPT, starting with the provider that is not orchestrating; trivial ones prefer the emergency providers. See [Teamwork features](features.md#orchestra).

## Quota balancing

Enable **Balance usage** in Auto and manage snapshots in Settings. Codex uses official `account/rateLimits/read`, refreshing missing/stale snapshots before routing. Claude percentages/reset times must currently be copied from `/usage`. Snapshots expire after 15 minutes. Balancing runs only when all candidates have fresh usable data; otherwise activity explains why ordinary routing was used. Missing usage never counts as free capacity.

For each active reported window:

`pressure = fraction_of_window_time_remaining / fraction_of_quota_remaining`

An account's pressure is its tightest window (maximum); lower pressure is preferred. Thus 90% weekly usage with two hours until reset can be preferable to 40% usage with days remaining, unless the shorter window is tighter. Exhausted active windows block selection, including as reviewer, when all accounts have known usage. Expired windows are ignored rather than assumed reset. This balances subscription quota percentages, not currency or exact tokens; model-specific buckets and task cost prediction are not inferred.

This is a Lumen heuristic inspired by remaining-budget pacing, not an implementation of Envoy or a proven cost optimizer. Existing global-quota systems regulate throughput; they do not directly solve subscription selection across providers with independent five-hour and weekly resets. See [Envoy global rate limiting](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/other_features/global_rate_limiting.html) and [official Codex App Server quota protocol](https://learn.chatgpt.com/docs/app-server#6-rate-limits-chatgpt).

## Workers and permissions

Writer → read-only reviewer → final writer and two read-only perspectives → synthesis are bounded worker workflows. They use separate official CLI calls, at most two simultaneously per job. Only the writer/final pass may edit, according to the permission profile. Another provider is required for independent review; a single provider uses one stage. Recursive/native subagent spawning and concurrent writers with isolated worktrees are not implemented.

Four orchestration jobs can be active, with at most eight inference processes. Writing jobs sharing a canonical folder are serialized. Extra stages consume quota and receive task context. Reviewer/synthesis failure retains available draft/activity and marks the turn failed. There is no automatic retry/account rotation after inference errors. Selected model, effort, strategy and balancing reason appear in activity.

## Shared skills and MCP

Settings imports complete skill folders containing `SKILL.md` into `<Lumen data>/shared/skills`, including scripts/resources. Every model receives an inventory with descriptions/paths and instructions to read relevant skills. Shared MCP configuration lives in `<Lumen data>/shared/mcp.json`: Claude receives `--mcp-config`; Codex receives converted TOML `mcp_servers` entries. Stdio commands/args/env and HTTP URLs/headers are supported. Shared MCP runs in Autopilot; other permission modes keep restricted tools. Authentication may require separate provider setup.

This shares portable skills and MCP definitions. Provider-specific plugin hooks, interfaces, OAuth storage and unsupported transports require adapters; they are not converted automatically. See [Claude MCP configuration](https://code.claude.com/docs/en/mcp).
