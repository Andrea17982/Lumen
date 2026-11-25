# Third-party notices

Lumen's original code/documentation is MIT licensed. Dependencies retain their own licenses. The committed Rust lockfile and upstream license files are the authoritative version/license inventory.

The stack includes Tauri, Rust, Tokio, rusqlite/SQLite, serde, reqwest, axum, tokio-tungstenite, image, and tiktoken-rs. The desktop client is being rewritten; its dependencies will be listed here when it lands.

## T3 Code command receipts

The development version of Lumen adapts the durable command-receipt design from [T3 Code](https://github.com/pingdotgg/t3code), created by Theo / pingdotgg and contributors, **Copyright (c) 2026 T3 Tools Inc.**, under the MIT license. The source is pinned to commit `375343aaf35133f659435554cd01634bab333a7d`.

Lumen ports the design to Rust/SQLite, adds payload binding and a restricted command list, and refuses automatic replay of uncertain interrupted claims. This is not an import of T3's TypeScript backend or a promise of exactly-once tool execution. [Source files and changes](src-tauri/resources/legal/t3code/ADAPTATIONS.md) and the [unchanged upstream MIT text](src-tauri/resources/legal/t3code/LICENSE.txt) are also bundled under `resources/legal/t3code/` in installed and portable packages. Credit does not imply endorsement.

## Windows metadata process supervision

Windows metadata process supervision additionally uses [process-wrap](https://github.com/watchexec/process-wrap) 10.0.1 (Apache-2.0 OR MIT) and the Microsoft `windows` Rust bindings (MIT OR Apache-2.0). Job Objects supervise metadata process lifetime; they do not change provider permissions.

## Bundled Moli

Lumen 0.8 bundles [Moli](https://github.com/lexmount/moli) **1.1.23**, licensed **Apache-2.0 OR MIT** according to its release metadata. The platform package retains `LICENSE-APACHE`, `LICENSE-MIT`, `license-metadata.json` and the upstream `licenses/` notices, including the Chromium BSD and Selenium notices distributed with that release. These notices remain beside the browser in installed/portable resources.

`scripts/prepare-moli.mjs` acquires the exact release assets pinned in `src-tauri/moli-release.json`, verifies their published SHA-256 digests and copies the package with its notices. Consult those bundled files for full applicable texts and component attribution; Lumen's MIT license does not replace them.

## MCP HTTP client and TLS

The 0.8 HTTP integration adds the official [Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk) `rmcp` 3.9.0, reqwest 0.13, rustls/platform verification and their locked dependencies, including AWS-LC cryptography. Upstream license/notice texts for the newly locked crates are reproduced under `resources/legal/` in installed and portable packages. The [source index](src-tauri/resources/legal/index.json) identifies exact versions, declared licenses, archive checksums and notice-file hashes. Texts retain their original bytes with a `.txt` filename suffix. Target-only Android/JNI crates without notice files in their archives are listed in the index but are not desktop package targets.

This supplements the existing notices and lockfiles; it is not a claim that all third-party components share Lumen's MIT license or a replacement for Moli's own notice bundle.

## Official API keychain and HTTPS

The 0.9 adapters use `keyring` 3.6.3 with explicit Windows Credential Manager, Apple Keychain and Linux Secret Service backends, plus reqwest 0.12 rustls HTTPS. Newly locked dependencies and their original notices are included in the same verified legal index; target-specific entries do not imply desktop validation on those targets. Keys are not distributed.

## Separately installed software

Claude Code, Codex, Antigravity, OpenCode, Cursor CLI and Ollama are not redistributed inside Lumen. Provider access, account terms and the user's installed software licenses apply independently. Microsoft WebView2 has its own runtime license. User-provided Godot addons and project assets are not bundled.

## Names and assets

Apple, Liquid Glass, Anthropic, Claude, OpenAI, ChatGPT, Codex, Google, Gemini, Antigravity, Ollama, Moli, Microsoft, Windows and PayPal belong to their respective owners. Compatibility or design inspiration does not imply endorsement or affiliation.

`src/brand/app-icon.svg` is the source for the Lumen platform icons generated with Tauri’s icon tooling. System fonts are referenced rather than redistributed. No provider logos are shipped.
