# Moli in Lumen: feasibility verified on 10 November 2026

Moli is feasible as Lumen's shared browser engine. It is an independent Rust browser kernel with V8, rather than a Chromium launcher. Its main advantage is avoiding layout/paint for document extraction and producing Markdown or semantic trees directly. It cannot make the model itself more intelligent or shorten inference independently of the browser workload. [Architecture and scope](https://github.com/lexmount/moli#architecture).

## What was actually tested

The official Windows x64 **v1.1.23** archive was downloaded without executing the installer script. Its SHA-256 matched the release asset digest: `674652caf89c217c28d988e8eac6e63b4b3991a8dc829124618ead374da11c26`. The binary reports `moli 1.1.23`. Other releases/platforms were not executed locally. [Release](https://github.com/lexmount/moli/releases/tag/v1.1.23).

`tests/moli-probe.mjs` connects Playwright over CDP to a private loopback Moli process. Twelve episodes per engine open a local page, execute JavaScript, fill a field, click through DOM evaluation and read the result. Layout mode also produces a valid PNG, visually inspected. All three paths passed:

| Local synthetic workload | Moli DOM | Moli layout | Chromium |
| --- | ---: | ---: | ---: |
| Connection/startup, one sample | 88 ms | 77 ms | 79 ms |
| Median episode, 12 samples | 60 ms | 59 ms | 90 ms |

This is about **1.5 times** the episode throughput in this small test, with no demonstrated startup advantage. It is not a statistically controlled browser-wide benchmark. RAM, site compatibility and full agent task latency were not measured in this probe. HTTPS Markdown extraction also returned real content from the Rust website. No cookies, existing browser profiles, provider credentials or cloud browser account were used.

To repeat, download/verify the official binary, set `LUMEN_MOLI_EXE` to its absolute path, then run `node tests/moli-probe.mjs`. The script closes its own browsers/processes and writes evidence below `.impeccable/review/`.

## Published performance versus guarantees

The project's public 192-URL crawl reports equal median elapsed time for Moli and Chrome (1.43 s), with median RSS of 73 versus 773 MiB. Its separate small agent workload reports lower startup/active time and memory. These are measurements published by the project's own authors, not independent guarantees. [Published benchmarks](https://github.com/lexmount/moli#benchmark).

The authors' separate resource report measures a common-completed task subset; savings on that subset must not be interpreted as a browser-wide success rate or a speedup of an entire Lumen conversation. [Resource methodology](https://github.com/lexmount/Lexbench-Headless-Browser/blob/main/docs/reports/resource-card-20260812.md).

Compatibility remains material: the published five-engine comparison reports 81.88% for an older Moli 0.1.1 against Chrome's 99.85%. Those figures are not a current v1.1.23 acceptance result. Moli explicitly excludes pixel parity with Chrome and high-fidelity Canvas/WebGL/media playback. It is suitable for retrieval and DOM automation; Godot/web-game visual acceptance should retain the real target runtime. [Comparison](https://github.com/lexmount/Lexbench-Headless-Browser/blob/kitesurf-eval/docs/reports/five-engine-report-20260813.md), [current scope](https://github.com/lexmount/moli#project-scope).

## Native integration contract

The implementation should use a Lumen-owned browser service, exposed to official CLIs through a **single shared MCP adapter**. Moli currently provides CLI, CDP and WebDriver endpoints; its WebMCP support is not itself a replacement for the model-facing MCP adapter.

1. Package or acquire a pinned platform binary with digest verification and license notices. Do not download or execute an unpinned latest installer on every turn. Keep the browser process out of the GUI process so page crashes do not kill Lumen.
2. Configure the adapter automatically for every applicable native account, including Orchestra workers. Use the same tool names and results for Claude, Codex, Gemini and Antigravity; preserve their authentication and original harness. Configuration alone is not provider acceptance: verify discovery, a real tool call, cancellation and resume on each installed CLI.
3. Use Markdown/semantic output without layout for reading/search. Start persistent CDP only for multi-step interaction; enable layout and relevant images/fonts when pixels are needed. Bound requests and output, return source URLs and saved screenshot references, and keep each conversation's browser context isolated. Cancellation and Quit must stop owned requests/processes; closing to the tray must keep active sessions alive.
4. Enforce Moli routing in the provider adapter as well as the instructions: disable/deny alternative built-in browsing where the official CLI supports doing so. Do not claim an absolute guarantee based solely on a system-prompt sentence. Under unrestricted Autopilot a shell can still launch another client; enforcing every possible network path would require changing that authorization model.
5. Respect real permission profiles. Page reading and navigation differ from submitting forms, uploads or arbitrary evaluation with page side effects. MCP annotations and approval handling must represent that distinction. Treat retrieved pages as data and keep credentials out of outputs.
6. Show browser activity, source links and images in the existing transcript. Unsupported pages, authentication walls or CAPTCHA are explicit results; do not silently switch engines when the user requires Moli-only browsing.

### The all-provider limit

Claude/Codex and the Google CLIs can receive browser tools through their native MCP integrations. The **current Lumen Ollama runner uses `ollama run` with text stdin/stdout**, not a tool-capable agent harness. Merely putting Moli on PATH or adding an MCP JSON file does not give that runner autonomous browsing.

Ollama documents tool calling through its local `/api/chat` interface. True autonomous browsing there needs a local tool loop and a tool-capable model, or an official agent harness around Ollama. That is an architectural extension to Lumen's CLI-only runner, not something to conceal in a configuration change. URL prefetch can supply web context to any text model, but it is not equivalent to interactive browser autonomy. [Ollama tool calling](https://docs.ollama.com/capabilities/tool-calling).

CDP compatibility also needs regression coverage rather than assuming Playwright parity: Playwright documents a lower-fidelity connection than its own protocol; the local probe validates only the methods it exercises. [Playwright CDP documentation](https://playwright.dev/docs/api/class-browsertype#browser-type-connect-over-cdp).

**Delivery boundary:** this change set contains research, the verified binary in ignored local verification storage and a repeatable feasibility probe. It does not yet route production Lumen browsing through Moli. Do not present the probe or this integration contract as an implemented all-provider browser.
