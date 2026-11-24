<div align="center">
  <img src="public/lumen.png" width="76" height="76" alt="Lumen">
  <h1>Lumen</h1>
  <p><strong>The AI coding subscriptions you already pay for, in one quiet window.</strong></p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/releases/latest">Download</a> ·
    <a href="docs/README.md">Docs</a> ·
    <a href="README.it.md">Italiano</a>
  </p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/actions/workflows/ci.yml"><img src="https://github.com/Andrea17982/Lumen/actions/workflows/ci.yml/badge.svg" alt="Build and tests"></a>
    <a href="https://github.com/Andrea17982/Lumen/releases"><img src="https://img.shields.io/github/v/release/Andrea17982/Lumen" alt="Latest release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT license"></a>
  </p>
</div>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/conversation-dark.png">
  <img src="docs/images/conversation.png" alt="Lumen with a finished task: the request, the steps folded into one line, and the answer with what changed and what was checked">
</picture>

I pay for Claude, ChatGPT and a couple of other things, and I got tired of deciding which one to open, pasting context between them and checking their work by hand. Lumen is what I wanted instead: you sign in once with each official CLI (Claude Code, Codex, Antigravity, OpenCode, Cursor, local Ollama, or an API key), write what you need, and it picks who does the work.

It's free, MIT licensed and still in alpha.

## What it does

- **Picks the agent for you.** The best one available right now; when a quota runs out, another one carries on without redoing what was finished.
- **Tells you what happened.** One answer at the end: what changed, what was checked, what failed, what's still open.
- **Stops when it should.** Risky or irreversible actions wait for you. Everything else doesn't.
- **Big tasks get a branch.** A bar above the conversation lets you publish it as a PR, apply it to your folder or throw it away.
- **Remembers the project.** Decisions go into `LUMEN.md`, which every agent reads.
- **Keeps running.** Close the window and the work goes on in the background.

Two things are just switches in Settings, each with one line of explanation: **Orchestra**, which splits a large task between your agents after you approve the plan, and **Code Mode**, which turns many tool calls into one script to save tokens.

<img src="docs/images/settings.png" alt="Lumen settings: language, style, theme, and the Orchestra and Code Mode switches">

## Download

Windows x64: the setup is on the [latest release](https://github.com/Andrea17982/Lumen/releases/latest). It isn't signed yet, so Windows will warn you; check it against `SHA256SUMS.txt`. macOS and Linux build from source for now.

## Build from source

```sh
git clone https://github.com/Andrea17982/Lumen.git
cd Lumen
node scripts/prepare-moli.mjs
npm ci
npx tauri build --config src-tauri/desktop/tauri.conf.json
```

Tests and the rest are in [development](docs/development.md).

## Good to know

Autopilot gives agents broad access to your machine and is not a sandbox. Lumen never touches your provider credentials: each CLI signs in on its own, and API keys go to the system keychain. [Validation](docs/validation.md) · [Security](SECURITY.md)

## How it's made

The code, tests and docs are written by Claude Code under my direction; the product decisions are mine. That's a disclosure, not a security audit.

Thanks to **Theo / pingdotgg, T3 Tools Inc. and the T3 Code contributors**: Lumen adapts parts of T3 Code's design. [Sources and changes](src-tauri/resources/legal/t3code/ADAPTATIONS.md) · [Licenses](THIRD_PARTY_NOTICES.md). No endorsement is implied.

Bugs and ideas go in [Issues](https://github.com/Andrea17982/Lumen/issues). If you want to support it there's [PayPal](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen); it doesn't unlock anything. — Andrea

MIT © 2026 Andrea17982. Not affiliated with Anthropic, OpenAI, Google or the other providers named here.
