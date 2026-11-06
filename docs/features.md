# Teamwork features (0.3)

Lumen 0.3 lets Claude Code and Codex work together. Every feature is one click or on by default, and runs through the official CLIs. Lumen never calls provider APIs or reads credentials.

## Both: one prompt, two answers side by side

Choose **Both** in the composer (available in **Ask** mode). Claude and ChatGPT get the same prompt in parallel. Each answer streams into its own column, and each column receives its own events.

Both is a read-only comparison in the interface. The backend can still isolate edits in `git worktree` lanes (`lumen/claude-<id>`, `lumen/chatgpt-<id>`) with one-click apply, kept for a future interface.

## Cross-audit

- **Audit with …** appears when you hover over an answer. It sends the answer to the other model for review. If the turn changed files, the reviewer sees only the `git diff -U3` of that turn, not whole files.
- The **⟲ toggle** in the composer turns on cross-audit for every answer. The other model reviews the answer, the writer revises it, and the cycle stops early when the reviewer replies `APPROVED`. Settings → Teamwork sets 1–3 rounds.

## Shared project context (`LUMEN.md`)

The **book icon** in the top bar opens the project panel:

- **Shared notes** are saved as `LUMEN.md` in the conversation folder and injected into every turn for both providers. A `verify: <command>` line sets the self-healing check, and `verify: off` disables it.
- **Attach files** lists project files with ripgrep's walker, so `.gitignore` is respected. Each file shows its approximate token weight. Lockfiles, build output and minified files are marked as skipped. Secret files such as `.env`, keys and `.ssh` are never listed.

Each turn replays the conversation, including turns from the other provider and a note of the files they changed. Switching models therefore keeps the context.

## Rate-limit fallback and status

Lumen recognizes rate-limit and quota errors in CLI output, for example `429`, usage limit or `try again in 2h`. It marks that account as cooling down until the parsed reset time, or for 15 minutes if no time is given. The same message then continues on the other provider without being lost, and the activity records the switch. Settings → Teamwork → **Switch on rate limit** turns this off.

The top bar shows a dot for each provider. Green means ready, amber means close to the quota and red means cooling down. Red includes a countdown. The tooltip shows the local 5-hour turn count.

## Rewind

Before any turn that may write files, Lumen takes an invisible snapshot of the git working tree. It uses a private index and a commit stored under `refs/lumen/checkpoints/`. Your staging area, HEAD and branches are never touched.

The **↺ button** on any of your messages first previews which files will be restored or removed. It then restores them, removes that message and everything after it, and puts your original prompt back in the composer.

## Self-healing

After a turn that edited files, Lumen runs the project check. That is the `verify:` command from `LUMEN.md` if one is set. Otherwise Lumen picks one automatically: `cargo check`, `tsc --noEmit` or `go build`. If the check fails, the compressed error output goes back to the writer, which fixes it. Lumen makes at most 3 attempts. Settings → Teamwork → **Self-healing** turns this off.

## Token savings

- **Estimate before sending**: a live badge under the composer shows `~3.4k tokens · Low/Medium/High`. It turns red when one attached file is more than 80% of the prompt or when the message contains a large encoded blob.
- **Spill store**: if you paste a log of more than 400 lines, Lumen sends only the error lines with context, at most 50 lines. The full log goes to `.lumen/spill/<hash>.log`, which is ignored by git automatically, and a one-line pointer tells the model where to find it.
- **Handoff & compact**: in long conversations, **Compact and continue** starts a fresh conversation. It is seeded only with the goal, recent results, the uncommitted `git diff` and the next step.
- **Context hygiene**: Claude is denied reads of `.env`, `.ssh`, `.aws` and the common lockfiles, even in Autopilot.

## Tools (MCP hub)

Settings → **Tools (MCP)** keeps one list of MCP servers for both CLIs:

- **+** adds a server from a command or URL.
- A switch turns each server on or off, and a dot shows whether its command was found.
- **Edit as JSON** opens the full configuration.

Lumen passes enabled servers to Claude (`--mcp-config`) and Codex (`-c mcp_servers.*`) whenever edits are allowed. Your global CLI configuration is never modified.

## Local API (OpenAI-compatible)

Settings → **Local API** → **Serve on localhost** starts `http://localhost:8317/v1` on 127.0.0.1 only. It serves:

- `GET /v1/models`
- `POST /v1/chat/completions`, as a full JSON response or as SSE when `"stream": true`

The available models are `lumen-auto`, which falls back across providers, `lumen-claude` and `lumen-chatgpt`. Requests run read-only in a private folder. Every request needs the API key shown in Settings. The panel shows recent requests.

```bash
curl http://localhost:8317/v1/chat/completions \
  -H "Authorization: Bearer <key from Settings>" -H "Content-Type: application/json" \
  -d '{"model":"lumen-auto","messages":[{"role":"user","content":"Hello"}]}'
```

Usage follows each provider's terms for its CLI and subscription.

## Cross-platform

Process launching uses the official executables directly: `.cmd` handling on Windows and process groups on Unix. Cancellation uses `taskkill /T` on Windows. On macOS and Linux it sends SIGTERM to the process group, followed by SIGKILL. On macOS and Linux the GUI loads the login-shell `PATH` at startup, so it finds `node`, `claude` and `codex`. **Sign in** opens Terminal on macOS, or the default terminal emulator on Linux.
