# Auto execution policy

The first version uses deterministic rules in `src-tauri/src/router.rs`. It does not send a preliminary classifier request, inspect your credentials, discover plan quotas or infer that more accounts mean more allowed usage.

| Signal | Result |
| --- | --- |
| Request under 240 bytes, no complex keyword | Prefer Codex, low effort, single call |
| Other ordinary request | Prefer Codex, medium effort, single call |
| More than 1,800 bytes or architecture/refactor/security/migration keywords (plus selected Italian forms) | Prefer Claude `opus`, high effort |
| Comparison/alternatives keywords, with both providers configured | Two read-only independent perspectives, followed by synthesis |
| Complex request, with both providers configured | Writer, other-provider read-only reviewer, final writer pass |
| Preferred provider not configured | First configured provider; a single provider cannot cross-review itself |
| Manual Claude / GPT | Chosen account/model/effort; single stage |

Comparison takes precedence over complex-task review. Codex uses its installed CLI default model unless manually overridden. Claude aliases (`opus`, `sonnet`) delegate concrete version selection to the CLI/provider. There is no unsupported claim that every account has access to these models. A manual model name is validated before process creation. Effort is restricted to low / medium / high for cross-provider compatibility.

## Sub-agents, review and pair work

Workers are separate official CLI calls, bounded to at most two in parallel per orchestration. Reviewers cannot edit files. Parallel perspectives cannot edit files; the final synthesis may edit only when the user enabled file edits. Workers receive the relevant transcript and drafts, so both configured providers can receive task content in Auto. Manual provider selection prevents the cross-provider stages.

This is a bounded form of autonomous delegation and pair/review. Recursive delegation, a dedicated multi-agent protocol, provider-native subagent spawning, dynamic agent counts and worktree-isolated parallel writers are roadmap work. Lumen does not silently pretend those exist.

Each job holds a semaphore permit. Four jobs can be active; parallel jobs can each start two provider processes, so eight is the maximum inference-process fanout. A job waiting for a workspace lock still holds its job permit. This favors simple correctness over optimal throughput.

## Costs, failures and transparency

Review and parallel plans can make three calls instead of one. They consume normal account quotas and may take longer. Account labels do not imply successful login, and the router does not rotate after failure. A failed reviewer/synthesis marks the turn as failed and retains any already-produced draft and activity. Model access, quotas and authentication errors require user recovery.

The UI displays the selected provider, effort and strategy; the plan also lives in persisted activity. No hidden background classifier or telemetry is implemented. More sophisticated complexity estimation, configurable budgets, live model discovery and measured routing quality belong to v1/v2 and need evaluations before becoming defaults.
