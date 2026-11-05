# Lumen

<!-- impeccable:product-schema 1 -->

## Platform
web

Desktop Windows application using a local Tauri webview. Linux is a secondary target.

## Stack
Required by Istruzioni.md: Tauri 2, Rust core, React + TypeScript, Tailwind CSS, Framer Motion.

## Users
People working with concurrent Claude Code and Codex sessions on their own computer.

## Product Purpose
One quiet workspace for AI conversations, with automatic routing and optional advanced controls.

## Capabilities and Constraints
Persistent searchable sessions, pins, streaming, collapsible activity, isolated account directories and official CLI authentication. No token copying, proxy or custom OAuth implementation. Auto chooses a bounded execution plan. Sessions have no artificial count limit; running processes must be bounded. Provider policies and actual hardware remain constraints.

## Brand Commitments
Lumen. Apple Liquid Glass, restrained controls, system typography, light and dark themes. Public GitHub repository under Andrea17982. Completely AI Written, with thanks to testers and optional PayPal support.

## Evidence on Hand
Istruzioni.md is the original brief. No existing implementation, benchmarks or testimonials.

## Product Principles
- Make the next action obvious.
- Keep orchestration inspectable without making it mandatory to configure.
- Let the official providers own authentication.
- Persist useful work and make failures recoverable.

## Open decisions
The user selected English for the entire interface. No performance or provider compatibility claims until verified.
