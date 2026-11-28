# How Lumen works

Lumen runs your request on the agents you have connected and reports only what it actually observed. This page covers the behavior. Providers and permissions are in [Providers](providers.md); internals are in [Architecture](architecture.md).

## First launch

On a new installation Lumen opens a guided setup. Every step can be skipped.

1. **Welcome.**
2. **Appearance.** Language, Ink or Satin, and light, dark or system theme. A live preview shows the result.
3. **Agents.** Lumen finds the installed CLIs and shows each account as ready, not connected or missing. Claude Code and Codex sign in without a terminal; the other CLIs open their own sign-in.
4. **Project.** Choose the folder you want to work in.
5. **Summary.** It lists what is ready and what is still missing. Auto stays the default.

A quick tour of the composer, the agent and access menus, Stop, the task bar, Orchestra and Maintenance can follow.

The service stores whether you finished or skipped each part. **Settings › General** opens the setup or the tour again. An existing installation that updates gets a small invitation instead of the full screen.

## Modes

- **Direct.** You pick a provider and model. Lumen never overrides that choice, even when the request mentions architecture or security.
- **Auto** (the default). Lumen classifies the task with a small model, falling back to keyword heuristics, and picks a model from the connected catalogs. A short continuation such as "procedi" reuses the previous rating. A new task gets a fresh rating. Every priority balances accounts of the same provider that offer the selected model and effort. With fresh, comparable quota data, Auto compares the fraction of time until reset against the fraction of quota remaining, starting with the tightest window. If the available model is weaker than the conversation's retained difficulty, Auto asks before using it and, when that is a quota trade, names the account that could serve the task and when its quota resets; an explicit provider or model choice is never questioned. When every Claude and ChatGPT account is unavailable, Auto continues on OpenCode ([details](providers.md#accounts)). Missing or stale quota data never counts as exhaustion.
  - **Save quota** lowers effort, skips optional stages and can also switch between Claude and ChatGPT for more quota if the other provider has a model in the same tier at equivalent or higher effort. Choose **Auto → Priority → Save quota** in the composer.
  - **Balanced** uses calibrated routing.
  - **Best quality** raises effort and adds independent review for tier-two tasks.
- **Orchestra** is opt-in. It plans a task graph with file ownership and dependencies, then runs workers in isolated Git worktrees within limits on concurrency, total subtasks, quota and run time (120 min by default, up to 24 h). It integrates the results and runs final validation.

| Shape | When |
| --- | --- |
| Single | Default. Adds a read-only reviewer when review applies, preferably from another provider. |
| Parallel | Orchestra plan whose steps can run side by side. |
| Collaborative | Every step depends on the earlier ones, or two writers claim one file. Agents take turns in plan order. |
| Compete | Best quality only. Applies to one important or uncertain writing task when two different providers have quota. |

How Compete works:

1. Both providers build the task in separate worktrees.
2. The project's checks run on each candidate.
3. The winner is decided by rule, in this order: no failing check, most checks passed, the verdict of a reviewer from a third provider, smaller diff.

In an offline corpus of six tasks, Compete solved 5/6 and Single 3/6, at twice the worker runs. Save quota and Balanced never compete.

Every Orchestra run ends with **Team result by Lumen**. Lumen writes this summary itself from observed results, not from model claims. It lists subtasks done and failed, the audit verdict, and says so when a writing task ran no check.

## Questions, quota and continuation

- **Questions in Auto.** Lumen answers clarifying questions from agents itself. It picks the option marked recommended, otherwise the first one, and lists its choices under **Assumptions Lumen made**. It still asks you about the following:
  - risky or external actions;
  - secret input;
  - permission, browser-action and connector requests.

  In the other modes every question waits for you.
- **Quota.** Exhausted or cooling accounts are excluded before routing. A runtime quota error offers the next model and asks before sending. It warns when that model is weaker. Once work has been observed, Lumen never replays it: partial output stays, and the account cools down.
- **Resume at quota reset.** This schedules one native continuation with the same account, model and permissions. The local service keeps it across window closes and restarts. The time is the one the provider states, such as Codex's "try again at 1:18 PM" or Claude Code's "resets 3pm". When the provider gives none, the button says the time is an estimate.
- **Continue on an equivalent model.** After a quota stop, Lumen can propose the same tier on another account, preferring another provider, for example Opus after GPT. Choosing it starts a new turn there with the same permissions. That turn reads the conversation and the files as they are, and nothing already done is sent again.
- **Continuation.** "Prosegui" inherits the previous task class. A weaker model requires your consent.
- **Codex Autopilot.** Lumen continues unfinished work in the same native thread while it observes new tool evidence. Repeated operations with the same outcome do not extend the run. It tries once to recover when progress stalls and pauses for your answer to any structured question. A run stops after 10 attempts at most. When the agent reports a blocker, progress stalls or the limit is reached, the reply and its work are kept, a note says why it stopped and the conversation is marked **Partial**: send a message to continue. Errors are reserved for real failures of Lumen or the provider.

## Task worktree and Publish

A large Auto task that writes files runs in the conversation's own worktree. Small tasks, Read only conversations and Orchestra runs stay in the project folder.

- **Setup.**
  - The branch is named `lumen/<first words>-<id>`.
  - Uncommitted changes become its first commit.
  - `.env*` and `*.local*` files up to 1 MB are copied in.
  - Dependencies are installed from the lockfile, in Autopilot only.
- **Commit after checks.** Every turn that ends without failing checks is committed. The message ends with `Generated-by: Lumen`.
- Enable **Show Publish PR and Undo changes** in **Settings › General**. **Publish PR** pushes the task branch to the GitHub repository configured as `origin` and opens or updates its pull request. **Undo changes** returns the conversation to the project and shows where the branch and task files are kept for recovery.
- **After Publish**, Lumen checks the pull request every minute.
  - Failed CI and change requests from people with write access come back as a correction turn. CI logs and review text are passed as data, not instructions.
  - Lumen stops after three rounds or after a correction that produces no commit.
  - It merges once all checks are green. When branch protection blocks the merge, it enables GitHub auto-merge instead.

Publish needs an `origin` remote and a signed-in GitHub CLI.

Press **Ctrl+Enter** to queue a message while a turn is active. Lumen starts queued messages in order after a successful turn. After Stop or an interruption, use **Return to composer** to recover a queued message.

## Memory, recipes, recurring tasks

- **Project memory.** An agent writes `<lumen-remember>fact</lumen-remember>`, and Lumen appends the fact, dated and with its source chat, to `## Lumen memory` in the project's `LUMEN.md`. Every agent reads that file. Limits: 5 facts per turn, 300 characters each, no duplicates, nothing that looks like a credential.
- **Global memory.** `scope="global"` only proposes the fact. It is saved to `shared/MEMORY.md` only after you click Save. Project notes win on conflict.
- **Untrusted content.** When a turn read web pages, third-party MCP output or network commands, Lumen holds proposed memory and routines for confirmation instead of saving them.
- **Recipes.** A saved reply becomes a shared `recipe-…` skill that any agent can repeat on the current state.
- **Recurring tasks.** Use `/every 1d …`, `/every 08:00 …` or `/every lunedì 09:00 …`, or ask in plain words. The interval is 15 min to 30 days. The daemon runs the task in the same chat with its current permissions. It skips a run while the chat is busy and never replays missed runs in a burst.

## Checks and corrections

After a write turn, Lumen diffs the working tree against that turn's checkpoint. Extra validation (`verifyChanges`) is opt-in for ordinary turns and always on for Orchestra.

1. Lumen collects diagnostics from installed language servers. The defaults are `rust-analyzer`, `typescript-language-server` and `csharp-ls`; `.lumen/diagnostics.json` overrides them.
2. It runs the affected project checks: Rust, JS package scripts, C#, Go and Godot. You can override them with `verify:` in `LUMEN.md` or with `.lumen/checks.json` (entries need `"automatic": true`).
3. It sends concrete errors back to the writer, for up to three corrections.

When verification is enabled, an independent reviewer sees the latest check results for the diff it reviews. Lumen also reuses the latest recorded evidence in a handoff. Failed checks include a compact excerpt. The verification step shows the captured log, within the output limit.

Missing coverage is reported as missing, never as a pass.

The code graph is pattern-based, not a compiler. It feeds pre- and post-edit impact (callers, tests, contracts) to workers and to integration.

## Recoverable autonomy

| Class | Examples | Behavior |
| --- | --- | --- |
| Reversible | Reads, project edits, builds, tests | Runs under the profile; rollback undoes it. |
| Risky | `rm -rf`, `git reset --hard`, `git push --force`, `DROP TABLE`, `terraform destroy` | Lumen's own tool loops (APIs, Ollama) always ask, even in Autopilot. |
| External | `git push`, `npm publish`, `gh pr create`, `curl -X POST`, `ssh`, DB clients, browser click/fill, third-party MCP | Runs under the profile. Logged as not undone by rollback. |

Classification (`autonomy.rs`) matches command words and tool names. **It is not a sandbox.** Native CLIs in Autopilot run their own tools: Lumen logs those actions but does not block them beforehand.

- **Audit log.** Every conversation has one, covering commands, tools, file changes, browser actions and Lumen's own decisions. Rollbacks are also written to `audit/<conversation>.jsonl`.
- **Checkpoints.** Before each writing turn, Lumen snapshots tracked and untracked files through a private Git index, byte-exact, with filter drivers disabled. It never touches your index, HEAD or branches.
  - Rewind first snapshots the state it is about to replace, so a rewind can itself be undone.
  - Rewind is refused while an overlapping conversation is running.
  - Rewind is refused when a later native turn ran outside the checkout.
- **Rollback does not undo** databases, ignored files, remote services, files outside the project, or running processes.

## Repair and updates

**Settings › Maintenance** holds the two things that keep Lumen itself working.

- **Repair Lumen** runs these checks:
  - **Service.** If the service does not answer, Lumen restarts it. It asks the service to stop first. Only if that fails does it end the process serving Lumen's pipe, and only when that process is a Lumen service. The same **Repair** button appears on the banner when the service is unreachable or another version is running.
  - **Database.** SQLite's integrity check runs, then a backup is saved in `<app-data>/backups`. The newest five backups are kept, and a damaged database is never copied over them.
  - **Conversations.** A conversation still marked running with no turn behind it is closed as interrupted. Its work is kept, never replayed.
  - **Agents.** Each agent's CLI is checked: installed, version, signed in.
  - **Task folders.** Task folders whose conversation was deleted, or that could not be removed earlier, are listed with their unsaved files. Removing one needs your confirmation. Its branch stays, so committed work is not lost.
- **Updates.** At startup and every six hours, Lumen checks the project's GitHub releases for a newer signed version.
  - It downloads nothing until you choose **Download and install**.
  - Lumen refuses while work is running. Otherwise it verifies the package's signature against the key built into Lumen, backs up the database, stops the service and installs. Lumen then restarts.
  - **Go back to** reinstalls the version the last update replaced, also signed. If that version cannot read the current database, the pre-update backup is restored and the newer database is kept in the backups.
  - The installed app updates itself. The portable ZIP and the Linux package show the new version with a link to its release page.
- **Export conversation…** in a conversation's More actions menu saves the whole conversation as a `.txt` file where you choose: each message, every tool call with its full output, errors and the execution diagnostics Lumen recorded. Agents cannot produce this themselves, because only the service keeps that history.

## Moli browser

Moli is the browser bundled with Lumen: pinned v1.1.23, verified by SHA-256. All agents share it through one MCP adapter, `lumen_browser`.

- It offers 8 tools: fetch, search (DuckDuckGo, then Brave), navigate, extract, click, fill, evaluate and screenshot.
- Each conversation or worker gets its own browsing context.
- Native browsing in Claude, Codex and Antigravity is disabled in favor of Moli.
- The broker enforces the access profile itself: Read only and File edits deny page mutations, and Ask first requires a decision for each action.
- Page content is untrusted data.
- Search can hit CAPTCHAs or consent pages, and Canvas/WebGL rendering is not at parity with Chromium.
