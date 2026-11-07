# Lumen

<!-- impeccable:product-schema 1 -->

## Platform
web

Desktop application using a local Tauri webview: Windows and macOS (Apple silicon); Linux builds from source.

## Stack
Stack: Tauri 2, Rust core, React + TypeScript, Tailwind CSS, Framer Motion.

## Users
People working with concurrent Claude Code and Codex sessions on their own computer.

## Product Purpose
One quiet workspace for AI conversations, with automatic routing and optional advanced controls.

## Capabilities and Constraints
Persistent searchable sessions, pins, streaming, collapsible activity, isolated account directories and official CLI authentication. No token copying, proxy or custom OAuth implementation. Auto chooses a bounded execution plan. Sessions have no artificial count limit; running processes must be bounded. Provider policies and actual hardware remain constraints.

## Brand Commitments
Lumen. Apple Liquid Glass, restrained controls, system typography, light and dark themes. Public GitHub repository under Andrea17982. Completely AI Written, with thanks to testers and optional PayPal support.

## Evidence on Hand
The original human brief defined the product; no benchmarks or testimonials are claimed.

## Product Principles
- Make the next action obvious.
- Keep orchestration inspectable without making it mandatory to configure.
- Let the official providers own authentication.
- Persist useful work and make failures recoverable.

## Open decisions
The interface supports English and Italian, with English as the initial language and the user's selection saved locally. No performance or provider compatibility claims until verified.

## Confirmed 0.2 changes
The user requested removal of alpha/promotional labels from the chat shell; named workspace plus per-conversation folder selection beside the AI selector; easy account renaming; model/effort selectors from official providers; and an explicit full-permission Autopilot mode. Authorship and thanks remain in About/README. Autopilot grants available CLI tools under OS permissions; it does not itself implement graphical computer control.
