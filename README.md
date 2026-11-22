<div align="center">
  <img src="public/lumen.png" width="76" height="76" alt="Lumen">
  <h1>Lumen</h1>
  <p><strong>Connect your AI accounts once. Write what you want. Auto does the rest.</strong></p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/releases/latest">Download</a> ·
    <a href="docs/getting-started.md">Getting started</a> ·
    <a href="docs/README.md">Docs</a> ·
    <a href="README.it.md">Italiano</a>
  </p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/actions/workflows/ci.yml"><img src="https://github.com/Andrea17982/Lumen/actions/workflows/ci.yml/badge.svg" alt="Build and tests"></a>
    <a href="https://github.com/Andrea17982/Lumen/releases"><img src="https://img.shields.io/github/v/release/Andrea17982/Lumen" alt="Latest release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT license"></a>
  </p>
</div>

![Lumen on Windows](docs/images/desktop.png)

Lumen is a desktop app for coding with AI. You sign in to the subscriptions you already pay for (Claude Code, Codex, Antigravity, OpenCode, Cursor, local Ollama or an API key) and Lumen uses them together, as one team, on your project.

It's free, open source (MIT) and still alpha.

## Auto

Auto is the reason Lumen exists. You don't pick a model, split the task or babysit the agents. You write the request and Auto decides:

- **who works on it** — the best agent you have available right now, and another one when a quota runs out, without repeating what was already done;
- **how** — one agent, one agent plus an independent reviewer from a different provider, or a full team that splits the work, runs in isolated worktrees and merges it back;
- **whether to compete** — in the Quality profile, important tasks get two independent solutions from different providers; Lumen runs your project's checks on both and keeps the one that passes (0.11);
- **what to do when an agent asks something** — Auto answers clarifying questions from the code and project memory and lists its assumptions at the end. It still stops for anything risky, irreversible or external (0.11).

At the end you get one answer: what was done, what was checked, what failed and what is still open. Lumen writes that part itself, from what it saw, not from what the agents claim.

Budget and Default profiles never run competing solutions, so a fresh install won't burn through your quota.

## Why I built it (and what the others do better)

I use several AI subscriptions and I was tired of choosing which one to open, copying context between them and checking their work by hand.

**Hermes Agent** — I love how it learns skills and keeps memory, and that you can talk to it from Telegram or Slack. It's better as a personal agent living on a server. Lumen is a coding workspace on your desktop that uses your subscriptions through their official CLIs and verifies the result against your project's checks.

**Conductor, Claude Squad and similar** — Great for running many agents in parallel worktrees.

**Cursor, the Codex app, Claude Code** — Polished, but each one keeps you inside one vendor. Lumen exists to make them work together.

Where Lumen is behind today: no mobile app yet, unsigned packages, and Windows is the only platform with full desktop testing.

**Code Mode measurements.** With Code Mode on, Moli and MCP tools are reachable only from a script. On Claude Code, reading twelve large files cost about 357k uncached tokens with direct tools and 37k from a script (−90%), with more correct answers (1/3 → 2/3). With small tool results scripts add a few requests, and Codex saved nothing. Local models through Ollama did not meet the bar for a default, so Code Mode stays opt-in. Subscription quota only; no API spend. [Results](reports/bench/2026-11-22/summary.md)

## Also inside

- **Memory** — project decisions go into `LUMEN.md`, which every agent reads; cross-project preferences are saved only after you confirm.
- **Undo** — checkpoints before risky changes, an audit log per task, and rollback that refuses to run when it would be unsafe.
- **Moli** — a built-in browser that all agents share for research and web tasks.
- **Recurring tasks and recipes** — "every Monday at 9, summarize the TODOs"; save a good reply as a reusable recipe.
- **Work survives the window** — a local background service keeps tasks running after you close Lumen.

More in the [feature guide](docs/features.md).

## Download

Get the latest version from **[Releases](https://github.com/Andrea17982/Lumen/releases/latest)**.

| Platform | File |
| --- | --- |
| Windows | `…_x64-setup.exe`, or the portable `…_x64-portable.zip` (extract the whole ZIP) |
| macOS | universal `.dmg` |
| Linux | `.deb` or `.AppImage` |

Packages are unsigned; check them against `SHA256SUMS.txt` in the release.

1. Install the official CLI of at least one provider (or start Ollama).
2. In **Settings → Accounts**, add your accounts and log in the official way.
3. Open a project folder, leave **Auto** selected and write what you need.

## Build from source

```sh
git clone https://github.com/Andrea17982/Lumen.git
cd Lumen
npm ci
npm run tauri dev
```

See [development](docs/development.md) for tests and release builds.

## Good to know

Autopilot gives agents broad access to your machine; it is not an OS sandbox. Lumen never copies your provider credentials: each CLI logs in on its own. What has been tested, and what hasn't, is in [validation](docs/validation.md). Security details are in [SECURITY.md](SECURITY.md).

## How it's made

Lumen's code, tests and docs are written by AI (Claude Code and Codex) under my direction. The product decisions are mine. This is a disclosure, not a security audit.

Thanks to **Theo / pingdotgg, T3 Tools Inc. and the T3 Code contributors**: Lumen adapts parts of T3 Code's design. [Exact sources and changes](src-tauri/resources/legal/t3code/ADAPTATIONS.md) · [licenses](THIRD_PARTY_NOTICES.md). No endorsement is implied.

Feedback and bug reports in [Issues](https://github.com/Andrea17982/Lumen/issues) help a lot. If you want to support the project, there's [PayPal](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen); it unlocks nothing extra. — Andrea

MIT © 2026 Andrea17982. Not affiliated with Anthropic, OpenAI, Google or the other named providers.
