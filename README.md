<div align="center">
  <img src="public/lumen.svg" width="76" height="76" alt="Lumen">
  <h1>Lumen</h1>
  <p><strong>Your AI workspace. Native sessions. A browser built for agents.</strong></p>
  <p>Claude Code · Codex · Gemini CLI · Antigravity · Ollama</p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/releases/latest">Download 0.8.0</a> ·
    <a href="docs/getting-started.md">Getting started</a> ·
    <a href="docs/README.md">Documentation</a> ·
    <a href="README.it.md">Italiano</a>
  </p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/actions/workflows/ci.yml"><img src="https://github.com/Andrea17982/Lumen/actions/workflows/ci.yml/badge.svg" alt="Build and tests"></a>
    <a href="https://github.com/Andrea17982/Lumen/releases"><img src="https://img.shields.io/github/v/release/Andrea17982/Lumen" alt="Latest release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT license"></a>
  </p>
</div>

![Lumen 0.7 conversation workspace](docs/images/desktop.png)

*A capture of the Windows application with demonstration data. Screenshots show the implemented interface; they are not claims of live model performance.*

Lumen brings your AI coding sessions into one local desktop workspace. Choose a provider and model, continue its native session, and work in the right project folder. Turn on **Orchestra** when you want a coordinated team. **Moli** handles web research and browser automation across every provider.

Built with **Tauri 2, Rust, React and TypeScript**, with English and Italian interfaces, light/dark themes, local conversation storage and official provider authentication. Lumen is an independent community project. Version 0.7 is an alpha release, not a claim of universal CLI or platform compatibility.

## 0.7.1 — Performance Update

Faster long-chat opening and typing, safer Windows metadata CLI shutdown, conversation-local drafts and clearer execution progress. The scoped Windows comparisons measure **25.6% faster opening** for 5,000 mixed messages and **37.2% / 57.4% lower complete metadata-call time** for the installed Claude/Codex CLIs. These are local benchmark results, not model-response latency guarantees. [0.7.1 release notes and limits](docs/release-notes/0.7.1.md).

## Native workspace features

| Native work, with less friction | Tools that stay close to your project |
| --- | --- |
| Direct sessions are the default; your provider/model choice is preserved. | Moli is bundled and automatically connected, including Orchestra workers. |
| Native questions, choices, child-agent output and real-time activity. | Project inspector, image zoom/grid, JSON/SQLite inspection and layout canvas. |
| Autopilot proceeds without repeated Lumen planning approvals. | Manual Both merges and side-by-side static worktree previews. |
| Native Claude/Codex continuation, durable task state and explicit folder changes. | Optional diagnostics, affected-project checks and opt-in Godot controls. |
| Work continues when you close to the notification area. | Windows, universal macOS and Linux packages. |

Read the [0.7.1 changelog](docs/release-notes/0.7.1.md), [feature guide](docs/features.md) and [validation record](docs/validation.md). The [0.7.0 changelog](docs/release-notes/0.7.0.md) retains the native-session/Moli release history.

## Choose how to work

**Direct session** — select Claude, GPT/Codex, Gemini CLI, Antigravity or Ollama. Pick an account, an available model and its supported reasoning level. Claude uses its bidirectional native stream; Codex uses app-server. Google providers retain their official print runners. Ollama uses its official local API.

**Auto** — ask Lumen to choose a provider/model from the connected catalogs, with Budget, Default or Quality preferences and optional independent review. Additional stages consume provider quota.

**Orchestra** — explicitly enable the orchestrator for larger tasks. Review dependencies and agent assignments, then let workers use isolated Git worktrees. Approved execution and Autopilot do not create another plan-approval loop. Limits, stop, integration and interrupted-run recovery are visible.

**Both** — compare Claude and Codex side by side in read-only mode. Retained worktree results can be inspected and merged through per-file/manual choices where applicable.

![Orchestra task graph and assignments](docs/images/orchestra-plan.png)

*Demonstration task graph in the actual interface. Native child-agent sessions and Orchestra workers are distinct capabilities.*

## One browser across providers

Moli starts only when a browser tool is needed. It provides Markdown retrieval, search, persistent navigation, DOM interaction and screenshots through Lumen's shared browser service. Conversation/worker contexts are isolated; native authentication remains with each provider.

Ollama gets a real tool/result loop through `localhost`, including a JSON-action fallback for conversational models without native function calling. No inference API key or remote Ollama service is required. Antigravity receives saved full-resolution image paths through its native file/image flow to avoid an observed inline-MCP image stall; Lumen still displays the complete PNG.

![Moli page capture in a conversation](docs/images/moli-browser.png)

*Moli browser capture displayed in Lumen. Website compatibility, CAPTCHA and rendering limits still apply. Moli is not a guarantee of better model reasoning or an exponential end-to-end speedup.*

See [browser tools, routing and measured limits](docs/moli.md).

## Inspect, refine and continue

![Project inspector and layout canvas](docs/images/inspector-canvas.png)

The project inspector brings source changes, project checks, visual review, image inspection, local web preview and a layout canvas beside the conversation. Godot controls are optional and verify the connected project before editor actions. Purchased addons are not bundled.

Context estimates use a local tokenizer; provider-reported usage remains distinct. A durable task ledger preserves requirements and decisions across truncation, Compact and Rewind. Native continuation sends the new request rather than replaying the entire conversation unnecessarily.

## Download and start

Get **[Lumen 0.7.1 from Releases](https://github.com/Andrea17982/Lumen/releases/latest)**:

| Platform | Package | Notes |
| --- | --- | --- |
| Windows x64 | `Lumen_0.7.1_x64-setup.exe` | Installer; uses Microsoft Edge WebView2. |
| Windows x64 portable | `Lumen_0.7.1_x64-portable.zip` | Extract the **whole ZIP**. Keep `resources/moli` beside `Lumen.exe`. |
| macOS | Universal `.dmg` | Intel and Apple silicon. |
| Linux x64 | `.deb` / `.AppImage` | Built on Ubuntu 24.04; native display/capture compatibility varies. |

Packages are unsigned. Verify downloads with the release's `SHA256SUMS.txt`. Provider CLIs and local Ollama models are installed separately; Moli is included.

1. Install the official provider CLI you want to use, or start local Ollama with an installed model.
2. Open **Settings → Accounts**, add an account and complete its official login. Ollama needs no login.
3. Choose a project folder and a direct provider. Expand provider options to choose the model/reasoning level.
4. Choose permissions and send your request. **Read only** is the initial permission mode; **Autopilot** authorizes the available tools under your OS account.
5. Enable Orchestra separately when the task benefits from coordinated workers.

[Setup and troubleshooting](docs/getting-started.md) · [Accounts](docs/accounts.md) · [Permissions and folders](docs/workspace-models-permissions.md)

## Build from source

Prerequisites: Node.js 22.12+ (or supported Node 24), stable Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
git clone https://github.com/Andrea17982/Lumen.git
cd Lumen
npm ci
npm run tauri dev
```

The build prepares the pinned, checksum-verified Moli binary and license notices automatically. `npm run dev` is a UI-only preview: it does not authenticate or generate AI responses. On Windows, `npm run build:release` creates the installer and portable ZIP in `builds/latest/`. See [development and release checks](docs/development.md) for all platforms.

## What has been checked

The release preflight covers **135 Rust tests, 60 TypeScript tests and 29 UI tests**, formatting/Clippy, Windows native fixture runs, real Codex and Antigravity browser turns, and a real local Ollama model consuming Moli results. CI and packaging cover Windows, macOS and Linux. Clean Windows installation includes an installed-browser/helper smoke check.

Claude live inference remains deferred after account quota exhaustion; Gemini CLI login is completed by the user. Real non-Windows desktop/accessibility/device testing and the broader account/version matrix remain explicit validation limits. Fixtures and successful builds are not substitutes for these checks. See [validation and the closed release scope](docs/validation.md).

Autopilot has broad access. Lumen does not add a common OS jail or encrypt its database; use OS permissions and disk encryption. Native CLIs retain their own policies. Moli routing disables supported built-in alternatives, but unrestricted shells/external tools are not an OS network sandbox. [Security and privacy](SECURITY.md).

## Documentation

- [Getting started](docs/getting-started.md) — install, first session, shortcuts and recovery.
- [Features and working modes](docs/features.md) — direct sessions, Auto, Orchestra, Both and project tools.
- [Moli](docs/moli.md) — browser integration, Ollama architecture and compatibility.
- [Projects, models and permissions](docs/workspace-models-permissions.md) — workspace changes and native controls.
- [Task quality](docs/task-quality.md) — durable context, diagnostics and impact-aware validation.
- [Architecture](docs/architecture.md) · [Development](docs/development.md) · [Release history](docs/release-notes/README.md).
- [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [Third-party notices](THIRD_PARTY_NOTICES.md).

## Completely AI Written

Lumen's application code, interface, tests and documentation are **completely AI Written**, generated by AI under Andrea's direction. Product vision and decisions are human. AI authorship is a disclosure, not an independent correctness/security audit. Later contributions should be credited accurately.

> Thank you to everyone who wants to try Lumen and share how it feels. Feedback, bug reports and ideas are welcome. — Andrea

Lumen is free to download, use and modify under the MIT license. Feedback through [Issues](https://github.com/Andrea17982/Lumen/issues) is already a contribution. Optional [PayPal support](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen) goes to **andreanastrobusiness1999@gmail.com** and unlocks no features.

MIT © 2026 Andrea17982. No affiliation with Apple, Anthropic, OpenAI or the other named providers.
