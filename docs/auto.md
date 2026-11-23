# Auto and Orchestra policy — 0.7

Auto is the initial mode. Direct provider sessions bypass Lumen's classifier/planner when Orchestra is off. Auto can route between available accounts; Orchestra is an additional opt-in setting that can plan complex work even with a manually selected coordinator. With Orchestra off, an explicit manual provider/model is not overridden merely because the request mentions architecture or security.

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

### Calibration

Review now permits another primary account on the same provider, preferring a different provider when available. Budget preserves requested or risk-driven review. Quality adds optional review for tier-two work rather than every ordinary coding task, and explicit opt-out suppresses review. A qualified `APPROVED` response cannot accept an audit. Quota/cooldown routing cannot silently drop an explicitly requested review: it reports unavailability before work begins; optional review loss is visible in the route reason.

Only recognized short continuations reuse the latest request's rating; unrelated short requests and newer unrated manual tasks do not inherit an older verdict. Fresh Balance quota usage selection takes precedence over native-account affinity. The fallback distinguishes difficult single tasks from implementation across multiple areas or concrete parallel deliverables. The model classifier remains heuristic and native child-agent behavior remains provider-specific; these changes are not a new universal delegation engine.

## Orchestra

Enabled Orchestra can handle complex Auto or approved multi-part work. Its coordinator uses an appropriate catalog model and preserves an explicit coordinator choice where supplied. A task graph records file ownership, dependencies and assignments; workers receive actual workspace/permissions, prior decisions and relevant graph/check context.

The final reply ends with **Team result by Lumen**, written by Lumen rather than the model: subtasks done out of total, which failed or did not run, whether the independent audit approved, and, when no project check ran on a writing task, that the combined result is not verified.

The scheduler enforces concurrency, admitted-subtask totals, a run time limit (120 minutes by default, configurable up to 24 hours; after it no new subtask starts, running ones finish and the rest are reported as not run) and optional quota thresholds. Finished work frees concurrency, not the total-admission budget. Workers use separate worktrees for writes, and integration/final checks operate on their combined result. A live run can be stopped; crash recovery requires explicit Resume. Autopilot and already approved execution do not ask for the same plan approval again.

Auto's choice is deterministic in two steps. The router picks Single or Review; with Orchestra on, a complex request becomes an Orchestra run, and the approved plan's shape then picks Parallel or Collaborative (a dependency chain, or two writers on one file). Collaborative turns run in plan order, apply every earlier turn's patch and see every earlier result; they never bypass the run's limits. Tests: `auto_team_mode_rules_are_deterministic` and the `collaborative_*` tests in `orchestra.rs`.

### Compete

Compete is the third shape. It runs only when every rule holds: the plan has a single writing subtask; the planner marked it complex (important) or could not name its files (uncertain); Auto is in its Quality profile (Compete uses about twice the quota, so Default (balanced) and Budget never compete); the subtask limit allows two; and two different providers that can write are available and not cooling down. API accounts are never Orchestra workers, so Compete spends no API budget. Lumen then gives the same task to two pinned candidates on different providers (Claude first, then ChatGPT, then others), each in its own worktree, under the run's normal time, concurrency, subtask and quota limits.

Lumen runs the project's own checks (the same validation as the verify step) in each candidate's worktree. When both pass, a reviewer from a third provider when possible, otherwise one not orchestrating, compares the two diffs read-only and ends with `WINNER: A` or `WINNER: B`. The winner is fixed by rule: no failing check, then most checks passed, then the reviewer's verdict, then the smaller diff, then plan order. Only the winner's patch is integrated; the final check, audit and self-heal run as usual. If no candidate passes, the closest is applied and repaired, and the result says so.

When an important or uncertain task cannot compete, Auto runs Single, with the integration audit when Review is on, and **Team result by Lumen** says `Compete: not run (reason)`. A Compete run's result names the winner, why it won and what the other candidate failed. Tests: `compete_rules_are_deterministic_and_fall_back`, `compete_winner_breaks_ties_by_checks_then_reviewer_then_diff` and `team_summary_names_the_compete_winner_or_the_fallback` in `orchestra.rs`.

**Measured offline.** `compete_corpus.rs` has six small tasks with their own tests. Two scripted candidates stand in for two providers, so this measures Lumen's selection, not model quality. Each candidate runs in a real worktree with real checks, and the applied result is checked again in the project:

| Task | Single (candidate A) | Compete |
|---|---|---|
| slugify | pass | pass |
| parse duration | fail | pass (A failed its check) |
| clamp | pass | pass (tie, smaller diff) |
| dedupe keeping order | pass | pass |
| roman numerals | fail | pass (A failed its check) |
| leap year | fail | fail (both failed) |
| **Total** | **3/6** | **5/6**, at twice the worker runs (12 vs 6) |

Orchestra makes only successful dependencies eligible, fails missing/failed dependency chains, and preserves consumed admission and attempt limits across Resume. Worker errors after observed work do not automatically replay; partial text and the reason remain visible, with quota cooldown preserved. Stop waits for provider process cleanup before removing worktrees. These are bounded offline Windows results, not live model-quality or universal delegation claims.

## Usage and fallback

Codex quota refresh uses its official protocol. Claude streamed rate-limit events update supported windows during turns; manual snapshots remain available. Antigravity usage is parsed from its native headless output. Only reported five-hour/weekly windows are displayed; missing windows are omitted. Quotas refresh on opening Usage rather than launching idle CLI polling. Antigravity includes Google model groups only.

Balancing compares fresh usable windows against their reset times; absent data falls back to ordinary routing. Emergency providers can be used when confirmed primary quotas are exhausted or a running turn hits a limit while alternatives are cooling down. The configured order is Antigravity, local Ollama after primary subscription options. This does not multiply provider allowances or authorize credential sharing.

A runtime quota error offers the next catalog model and asks before sending, warning when it is weaker. Repeated failures can continue through remaining available accounts, with separate approval for every attempt. A decline stops without another request. Work or partial output already observed prevents replay. Explicit instructions such as “Usa antigravity per questo prompt” bypass classification and pin that provider. Known exhausted/cooling accounts are excluded before ordinary Auto selection. Provider changes retain task context. It is distinct from local-helper failures, which preserve the manual flow without invoking remote inference.

## Shared context and tools

Project `LUMEN.md`, attachments, durable task state and relevant skills/graph context are added according to the execution path. Native continuation avoids needless full replay. Shared skills and configured MCP adapters retain their provider-specific constraints. Moli is injected through the supported common invocation paths; OpenCode receives Moli/graph/MCP through its ACP session; Cursor retains its native tools without shared-tool injection. Official API accounts use the common host tool loop and require explicit Auto opt-in plus sufficient reserved budget. [API settings](api-access.md).

[Features](features.md) · [Accounts](accounts.md) · [Task quality](task-quality.md) · [Validation](validation.md)

## Continuation and quota reset

`Prosegui.` retains the previous task's class, including manual and legacy model choices, instead of being rated as an easy new request. Unrelated new tasks reset that inheritance. Auto requests consent before using an available model below the retained class. OpenCode and Cursor are currently selected explicitly; they are excluded from automatic selection/fallback.

After an eligible quota interruption, choose **Resume at estimated quota reset** on the latest reply. This schedules one native continuation with the same account/model/permissions and preserves already observed effects. It can be cancelled, occupies no inference slot while waiting and is kept by the local service, so it survives closing the window and service restarts. A missing native session stops without replay.

## Questions in Auto

When Auto is the selected mode, Lumen answers an agent's clarifying questions itself (Claude `AskUserQuestion`, Codex `requestUserInput`, the API agents' `lumen_ask_user`, and the provider-limit and model-capability `switch` questions). The answer is deterministic: the option marked "Recommended", otherwise the first option; a free-text question gets an instruction to proceed with the most reasonable assumption consistent with `LUMEN.md`, the code and the task. Each answer is recorded and listed at the end of the reply under **Assumptions Lumen made**, so you can correct it in your next message.

Lumen still asks you when:

- the question or every offered option names a risky or external action, as classified in [Recoverable autonomy](autonomy.md#action-classes) (a risky option is never picked automatically);
- the question asks for secret input;
- it is a permission, Gatekeeper, browser-action or connector request: those follow the permission profile and are never answered automatically.

Other modes are unchanged: every question waits for you. Orchestra workers started on their own task are not covered yet and still ask you.

