<div align="center">
  <img src="public/lumen.png" width="76" height="76" alt="Lumen">
  <h1>Lumen</h1>
  <p><strong>Connect your AI accounts once. Write what you want. Auto does the rest.</strong></p>
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

Lumen is a desktop app for coding with AI. You sign in to the subscriptions you already pay for (Claude Code, Codex, Antigravity, OpenCode, Cursor, local Ollama or an API key), and Lumen uses them together, as one team, on your project. It's free, open source (MIT) and in alpha.

> **Status:** the desktop interface is being rewritten from scratch. This branch contains the daemon and backend; the published releases still ship the previous interface.

## Auto

You don't pick a model, split the task or babysit agents. Auto decides:

- **who works on it**: the best agent available now, and another one when a quota runs out, without repeating finished work;
- **how**: one agent, one agent plus a reviewer from another provider, or a team working in isolated worktrees;
- **whether to compete**: in the Quality profile, two providers solve an important task independently and the solution that passes your checks wins;
- **what to answer**: clarifying questions are answered from the code and project memory, with the assumptions listed at the end. Risky, irreversible and external actions still stop and ask you.

You get one answer, written by Lumen from what it observed: what was done, what was checked, what failed and what is still open. [How Lumen works](docs/product.md)

## Also inside

- **From task to merged PR.** A large task runs in its own worktree. Lumen commits after the checks pass; **Publish** opens the PR, and Lumen then fixes failing CI and review comments and merges.
- **Memory.** Project decisions go into `LUMEN.md`, which every agent reads. Cross-project preferences are saved only after you confirm.
- **Undo.** Checkpoints before every write, an audit log, and a rollback that refuses to run when it would be unsafe.
- **Moli.** A bundled browser shared by every agent.
- **Code Mode.** Many tool calls turn into a single script, saving tokens on large results. [Measurements](docs/validation.md#code-mode-measurements)
- **Background service.** Work keeps running after you close the window.

## Why

I pay for several AI subscriptions and was tired of choosing which one to open, copying context between them and checking their work by hand. Cursor, the Codex app and Claude Code are polished, but each keeps you inside one vendor. Conductor and Claude Squad run agents in parallel, but you still drive them. Lumen makes them work together and verifies the result against your project's checks.

## Download

Get the latest release from [Releases](https://github.com/Andrea17982/Lumen/releases/latest): Windows setup or portable ZIP, universal macOS `.dmg`, Linux `.deb` or `.AppImage`. Packages are unsigned; verify them against `SHA256SUMS.txt`. Only Windows has full desktop testing.

## Build from source

```sh
git clone https://github.com/Andrea17982/Lumen.git
cd Lumen
node scripts/prepare-moli.mjs
cargo build --manifest-path src-tauri/Cargo.toml -p lumen-daemon --locked
```

See [development](docs/development.md) for tests.

## Good to know

Autopilot gives agents broad access to your machine and is not an OS sandbox. Lumen never copies your provider credentials: each CLI logs in on its own. [Validation](docs/validation.md) · [Security](SECURITY.md)

## How it's made

Lumen's code, tests and docs are written by AI (Claude Code) under my direction. The product decisions are mine. This is a disclosure, not a security audit.

Thanks to **Theo / pingdotgg, T3 Tools Inc. and the T3 Code contributors**: Lumen adapts parts of T3 Code's design. [Sources and changes](src-tauri/resources/legal/t3code/ADAPTATIONS.md) · [Licenses](THIRD_PARTY_NOTICES.md). No endorsement is implied.

Feedback in [Issues](https://github.com/Andrea17982/Lumen/issues) helps a lot. To support the project there's [PayPal](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen); it unlocks nothing extra. — Andrea

MIT © 2026 Andrea17982. Not affiliated with Anthropic, OpenAI, Google or the other named providers.
