# Lumen

A local Tauri 2 desktop workspace for native AI conversations, with optional Auto routing and explicit Orchestra coordination. Windows x64 is the primary acceptance target; macOS and Linux have automated builds with manual validation limits recorded separately.

<!-- impeccable:product-schema 1 -->

## Platform
web

## Stack
Tauri 2, Rust, React, TypeScript, Tailwind CSS and Framer Motion; SQLite persistence, official provider CLIs, loopback Ollama and pinned bundled Moli browsing.

## Users
People developing with Claude Code, Codex, Antigravity, local Ollama, OpenCode, Cursor and the official API adapters in their own project folders. React/TypeScript/JavaScript and Godot are priority workflows; multiple subscriptions may share one Lumen conversation.

## Product Purpose
Preserve the user's model, identity, permissions and task context while keeping conversation quiet. Auto selects from available catalogs. Target for 1.0: after one consent at setup, Auto moves between all connected accounts, including several of the same provider, before observed work and only to an equal or stronger model; with only weaker models it resumes at quota reset. Today each runtime fallback still asks before sending and warns about a weaker model. Either way it stops if work has already happened. Explicit provider instructions take precedence over Auto's preference. Google uses only Antigravity and available Google models; Gemini CLI is retired.

## Capabilities and Constraints
Searchable persistent conversations, project folders, native continuation/questions where supported, shared tools and browser access, bounded checks, images and project inspection. Orchestra is an explicit option for coordinated tasks, not the default route. Provider authentication stays in the official CLI; Lumen does not extract credentials or implement provider OAuth. Ollama stays local and rejects cloud models.

OpenCode uses one native profile, official ACP, verified session permissions and shared Moli/graph/MCP; Cursor retains explicit Autopilot-only native tools. Both remain explicitly selected. The three official API adapters use separate OS-keychain credentials, confirmed model pricing and durable budget reservations. Paid Auto access requires its own opt-in; budgeted API collaboration belongs to the later collaboration milestone.

Read only is the initial permission profile. Autopilot has the available OS-user access, not administrator privileges or a universal sandbox. Provider capabilities differ. Retry after quota or a missing session is permitted only before observed model work; partial output and file effects must never be duplicated.

Project folders organize conversations independently of filesystem workspaces. Project deletion removes contained conversations after confirmation and preserves actual project files. Shared integration discovery/execution covers a documented subset, not universal compatibility with every proprietary extension.

## Brand Commitments
Lumen uses the approved satin-glass lm logo, restrained Liquid Glass, system typography and English/Italian UI. The approved refinement keeps the floating composer and minimizes permanent diagnostic text. Authorship, tester thanks, MIT/open-source disclosure and optional PayPal support stay in About and repository materials. [Design system](DESIGN.md).

## Product Principles

- Preserve explicit provider/model, identity, folder and permission choices.
- Complete already authorized work without repeated approvals; provider changes still require the user's decision.
- Keep direct work direct; use Orchestra only when selected.
- Distinguish actual results from estimates, recorded observations and unverified claims.
- Continue in the local daemon when the GUI closes; recover uncertain interrupted execution only on explicit Resume. Explicitly scheduled quota intentions persist across service restarts.
- Prefer measured improvements and existing components to speculative infrastructure.

## Direction to 1.0

Reliability and intelligent collaboration take priority over breadth. Complexity belongs in the architecture, not in the interface.

- Guaranteed fundamental flows for Claude Code, Codex, Antigravity and OpenCode. Cursor/Ollama keep declared support and limits; no universal native-parity claim.
- All three official API integrations (Anthropic/OpenAI/Google) with OS-keychain storage. Auto requires explicit API enablement and a mandatory budget.
- A local service owns sessions (delivered in 0.10), with automatic project memory and user-confirmed global memory carrying their sources, and verifiable handoffs between providers.
- Auto may launch review/collaboration within configured limits. High autonomy includes audit log and recovery, with explicit boundaries for irreversible/external actions.
- Essential remote control as responsive web/PWA over LAN/VPN: chat, new tasks, Stop/Resume and approvals. A dedicated remote security gate remains mandatory. Full mobile workspace, native apps and E2EE relay move after 1.0.
- Preconfigured profiles, custom installation and in-app updates. Fixtures and native harnesses instead of live benchmarks. No inference proxy or credential harvesting.
- Free open-source core; future paid perks cannot lock fundamental features, including remote access.

## Release scope and evidence

The current release scope is in the [0.12.0 changelog](docs/release-notes/0.12.0.md); evidence and known limits are in [validation](docs/validation.md) and the [integration support matrix](docs/integrations.md).
