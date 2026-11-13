# Auto and Orchestra policy — 0.7

Direct provider sessions are the default and bypass Lumen's classifier/planner when Orchestra is off. Auto is an explicit routing mode; Orchestra is an additional opt-in setting that can plan complex work even with a manually selected coordinator. With Orchestra off, an explicit manual provider/model is not overridden merely because the request mentions architecture or security.

## Classification and model selection

Auto can use the enabled local Ollama helper or a small Claude model for classification. A bounded failure/timeout falls back to keyword heuristics. Short follow-ups can reuse the previous verdict so “continue” does not start an unrelated plan.

| Task | Typical model class | Effort tendency |
| --- | --- | --- |
| Short text or translation | Smaller available model | Low |
| Coding/debugging | Mid-tier coding model | Low/medium |
| Demanding architecture, migration or security | Stronger available model | Proportional to task complexity |

IDs, defaults and reasoning levels come from connected catalogs. Budget lowers effort and suppresses optional extra stages; Default uses calibrated routing; Quality raises effort and may add independent review. These are heuristics, not guarantees about task quality or cost.

## Review and comparison

In released 0.7, independent review needs another configured provider. Writer/reviewer/final or comparison/synthesis stages are bounded, and their actual model/effort appears in activity. The writer follows the selected permissions; reviewers stay read-only. Native child agents remain available where the provider protocol supports them.

### 0.8 working-tree calibration

Review now permits another primary account on the same provider, preferring a different provider when available; Both still requires Claude plus ChatGPT. Budget preserves requested or risk-driven review. Quality adds optional review for tier-two work rather than every ordinary coding task, and explicit opt-out suppresses review. A qualified `APPROVED` response cannot accept an audit. Quota/cooldown routing cannot silently drop an explicitly requested review: it reports unavailability before work begins; optional review loss is visible in the route reason.

Only recognized short continuations reuse the latest request's rating; unrelated short requests and newer unrated manual tasks do not inherit an older verdict. Fresh Balance quota usage selection takes precedence over native-account affinity. The fallback distinguishes difficult single tasks from implementation across multiple areas or concrete parallel deliverables. The model classifier remains heuristic and native child-agent behavior remains provider-specific; these changes are not a new universal delegation engine. See [experiment evidence](quality-experiments-0.8.md) for controlled tests, live budget and limitations.

## Orchestra

Enabled Orchestra can handle complex Auto or approved multi-part work. Its coordinator uses an appropriate catalog model and preserves an explicit coordinator choice where supplied. A task graph records file ownership, dependencies and assignments; workers receive actual workspace/permissions, prior decisions and relevant graph/check context.

The scheduler enforces concurrency, admitted-subtask totals and optional quota thresholds. Finished work frees concurrency, not the total-admission budget. Workers use separate worktrees for writes, and integration/final checks operate on their combined result. A live run can be stopped; crash recovery requires explicit Resume. Autopilot and already approved execution do not ask for the same plan approval again.

The 0.8 continuity checkpoint makes only successful dependencies eligible, fails missing/failed dependency chains, and preserves consumed admission and attempt limits across Resume. Worker errors after observed work do not automatically replay; partial text and the reason remain visible, with quota cooldown preserved. Stop waits for provider process cleanup before removing worktrees. These are bounded offline Windows results, not live model-quality or universal delegation claims. [Protocol, reproduction and limits](auto-orchestra-continuity-0.8.md).

## Usage and fallback

Codex quota refresh uses its official protocol. Claude streamed rate-limit events update supported windows during turns; manual snapshots remain available. Antigravity usage is parsed from its native headless output. Missing, stale and partial data are reported explicitly rather than assumed available.

Balancing compares fresh usable windows against their reset times; absent data falls back to ordinary routing. Emergency providers can be used when confirmed primary quotas are exhausted or a running turn hits a limit while alternatives are cooling down. The configured order is Antigravity, Gemini CLI and local Ollama after primary subscription options. This does not multiply provider allowances or authorize credential sharing.

Optional fallback records provider changes and carries task context. It is distinct from local-helper failures, which preserve the manual flow without invoking remote inference.

## Shared context and tools

Project `LUMEN.md`, attachments, durable task state and relevant skills/graph context are added according to the execution path. Native continuation avoids needless full replay. Shared skills and configured MCP adapters retain their provider-specific constraints. Moli is automatically injected through the common invocation path, regardless of the selected mode/provider/model.

[Features](features.md) · [Accounts](accounts.md) · [Task quality](task-quality.md) · [Validation](validation.md)
