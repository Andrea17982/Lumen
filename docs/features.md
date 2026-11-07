# Teamwork features

Lumen lets Claude Code, Codex and the other connected CLIs work together. Every feature is one click or on by default, and runs through the official CLIs. Lumen never calls provider APIs or reads credentials.

## New in 0.5.0

- **Project context panel removed** from the conversation header and command palette because its sections overlapped and could not be read reliably.
- **Attach files from the PC** with the composer paperclip: choose multiple files of any extension. Images pasted with Ctrl+V are saved as attachments. Images appear in the transcript; readable small text files are included in the prompt, while binary or large files are referenced by path for CLI tools.
- **Model-requested screenshots (Claude Ask-first)**: the model can list visible monitors/windows and explicitly call `capture_screen`. The image is returned to the model and saved in the project's ignored screenshot folder. Lumen never captures a screen automatically. User-started Visual review remains available.
- **Project write coordination**: write turns in conversations targeting the same folder are serialized. A waiting turn records which other conversation is active, and its Rewind checkpoint is taken only after it obtains the project lock. Rewind and worktree apply are blocked while another conversation is active; Rewind also refuses checkpoints followed by activity in another conversation on that folder.
- **Orchestra multi-task routing**: three or more checklist items, or explicit long requests for multiple parallel tasks/features, route through Orchestra when it is enabled.

- **Context indicator** beside Usage: a dot that is green under 50%, orange to 80% and red above. Clicking it shows the exact count (tokens used of the model's window), its parts and **Compact into a fresh session**. For Claude, the window and the CLI's own instructions and tools come from what Claude reported on its last reply; other providers show a local estimate.
- **Repository map references** (Settings → Teamwork, on by default): when a request names a declaration, its location is added to the prompt (at most 12, about 600 tokens) and shown under the request.
- **Screenshots** are never captured automatically. In Claude Ask-first turns, the model can request a screen or a specific visible window through MCP when visual inspection is needed. The composer has no screenshot button; user initiated capture remains in Visual review.
- **Settings** Simple mode contains only the Orchestra toggle. All other settings are under Advanced. General → Restore defaults resets local preferences and backend preferences while keeping accounts, conversations and project folders.
- **Godot**: **Test connection** starts the project's Godot MCP server and reads the editor's project info; **Open editor** opens a nested project in the installed Godot.
- **Checks**: a built-in `res://` reference check per Godot project, with git-ignored local files reported apart. Checks are marked headless or playtest (`"playtest": true` in `.lumen/checks.json`).
- **Change review**: uncommitted files grouped by layer (client, domain, backend, database, content, simulation, tests, docs) with the checks that cover each layer. `.lumen/layers.json` and `"layers"` on custom checks refine it.
- **Approve each action for Codex**: Codex turns in this mode run over `codex app-server`; each command or file change waits for approval in the chat panel.

## New in 0.4.2

- **Working indicator** under the last answer while an agent works: _Working_, _Thinking it through_ or _Calling a tool_, with animated dots.
- **Reasoning in a disclosure**, closed by default, instead of inside the answer. Inline `<think>` blocks from local models are separated too.
- **Commit message draft** above the composer, generated from uncommitted changes. **Copy message** copies it; Lumen never commits.
- **Editable handoff draft** on long conversations. **Continue in a fresh session** uses your revised text.
- **Ollama titles**: after the first answer, chats with a derived title get a short title from the local model. Renamed chats are never touched.
- **Orchestra limits** in Settings → Teamwork: simultaneous agents, total subtasks per run and a per-provider quota threshold. Waiting subtasks show "Queued" and start when a slot frees. Ollama is not listed.
- **Orchestra Resume**: a run interrupted by closing Lumen shows **Resume** or **Discard** at the next start. Completed results are kept, and attempts already used still count.
- **Approve each action** (Claude): a permission mode where every write or command asks for approval in the chat panel. Codex support came after 0.4.5.

## New in 0.4

### Orchestra

Settings → Teamwork → **Orchestra** is a set-and-forget switch that works with **Auto**. When it is on, a small **Orchestra** badge appears next to Auto in the composer; clicking it turns Orchestra off.

Orchestra takes over only when Auto scores a request as complex: architecture, security or migration work, or coding that is demanding or long. Simple prompts take the normal path.

1. **Questions.** One orchestrator model reads the request (and the project, read-only) and asks what it needs to know. Each question comes with its recommended answer already filled in. **Continue** sends your answers, **Skip questions** accepts the recommendations. When the request is already clear, it asks a single "I'd do X, Y, Z — agree?" with **Agree** or **Change**.
2. **Plan.** The orchestrator splits the work into subtasks, each with the files it owns, a difficulty and any dependency. Lumen assigns every subtask to an agent from all connected accounts that can load their models (a signed-out account is skipped), with the same model tiers Auto uses. Trivial work (messages, text, tiny edits) may go to Antigravity, Gemini CLI or Ollama even when Claude and ChatGPT have quota; Ollama only takes subtasks that write no files. The plan card lets you reassign an agent or remove a subtask, then **Approve** or **Cancel**. Nothing runs before you approve.
3. **Run.** Subtasks run in parallel, up to six at a time, each in its own git worktree seeded with your uncommitted work. Every agent receives the team manifest (who owns which files), so it stays aware of the others. A subtask that depends on another starts from that subtask's changes. The card shows each subtask's agent, status and elapsed time, with its live output in a disclosure. The stop button stops everything.
4. **Integrate.** Lumen applies each subtask's changes to your folder. If a patch no longer applies cleanly, the orchestrator merges it by hand. The orchestrator then checks the combined diff for inconsistencies, fixes them and writes the final answer. Self-healing runs on the integrated result and the worktrees are removed.

The orchestrator model follows the Auto preference: **Budget** and **Default** use Auto's tier for the task, never below Sonnet/Terra; **Quality** uses the strongest model (Opus/Sol).

With **Cross-audit** on, the provider opposite the orchestrator validates its decisions: the integration only with Budget; the plan (once, then the orchestrator revises it) and the integration with Default and Quality. Integration audits use the configured number of rounds.

If a subtask fails or hits a rate limit, Lumen retries it once on another provider. If that also fails, the rest is still integrated and the final answer lists what failed. If integration itself fails, the work is kept in `lumen/orchestra-*` branches: **Keep branches** leaves them for a manual merge, **Discard** deletes them.

If you write a new message while questions or a plan are waiting, the waiting card is marked as replaced and can no longer be used. A run interrupted by closing Lumen is marked **Interrupted** at the next start, and its temporary worktrees are removed. Rewind on "Approved the plan." restores your files and reopens the plan, so you can change it and approve it again.

Orchestra needs a git repository to isolate the subtasks. In a folder without git, the request runs normally and the answer offers **Initialize git**. In **Ask** (read-only) mode the agents only analyze: the plan card says so, and the orchestrator combines their findings. One Rewind checkpoint is taken before the run, on the "Approved the plan." message.

### Ollama

Add **Ollama (local, emergency)** in Settings → Accounts. Lumen uses only the official `ollama` CLI (`ollama list`, `ollama run`): there is no sign-in and nothing is isolated, because models live in your own Ollama store. The account row shows a status dot and the model count; **Install Ollama** appears when the CLI is missing, and an empty store suggests `ollama pull llama3.2`. Lumen uses the largest installed model; in Advanced mode a **Model** menu pins another one.

Ollama never edits files: it receives the prompt and returns text.

### Antigravity in File edits

Antigravity's command line, run without a terminal, refuses every shell command it would need to look around a project, even `dir`, and its allow rules cannot permit read-only commands alone. In **File edits**, Lumen therefore allows Antigravity to run **any command** without asking (Claude and ChatGPT can only edit files there). Choosing File edits while an Antigravity account is connected shows a one-time notice with **Don't show again**. Antigravity only runs as an emergency fallback or for Orchestra subtasks assigned to it. It is the last emergency fallback, after Antigravity and Gemini CLI, and an Orchestra agent for trivial text subtasks.

### Settings: Simple and Advanced

Settings is split into sections — **General · Accounts · Auto · Teamwork · Tools · About** — shown one at a time between a fixed header and footer. The **Simple | Advanced** switch in the header controls disclosure. Simple shows only the Orchestra toggle. Advanced shows every other setting. General → **Restore defaults** resets local and backend preferences while keeping accounts, conversations and project folders.

### Notification area and login

Settings → General → **Window**:

- **Keep Lumen in the notification area when closed.** The X hides the window and running responses continue. Left-click the tray icon to open Lumen; its menu has **Open Lumen** and **Quit**. The first time, a notification says Lumen is still running.
- **Open Lumen at login.** Registers a login item (Windows Run key, macOS login item). With the tray option on, Lumen starts hidden in the notification area.

With the tray option off, closing the window while a response runs asks "A response is still running. Quit anyway?". Quit cancels running CLI processes as before.

## 0.3

## Both: one prompt, two answers side by side

Choose **Both** in the composer (available in **Ask** mode). Claude and ChatGPT get the same prompt in parallel. Each answer streams into its own column, and each column receives its own events.

Both is a read-only comparison in the interface. The backend can still isolate edits in `git worktree` lanes (`lumen/claude-<id>`, `lumen/chatgpt-<id>`) with one-click apply, kept for a future interface.

## Cross-audit

- **Audit with …** appears when you hover over an answer. It sends the answer to the other model for review. If the turn changed files, the reviewer sees only the `git diff -U3` of that turn, not whole files.
- The **⟲ toggle** in the composer turns on cross-audit for every answer. The other model reviews the answer, the writer revises it, and the cycle stops early when the reviewer replies `APPROVED`. Settings → Teamwork sets 1–3 rounds.

## Shared project context (`LUMEN.md`)

If `LUMEN.md` exists in the conversation folder, its shared notes are injected into every turn for both providers. A `verify: <command>` line sets the self-healing check, and `verify: off` disables it. The Project context panel was removed; edit `LUMEN.md` with the preferred editor. Use the paperclip to attach files explicitly from any folder.

Each turn replays the conversation, including turns from the other provider and a note of the files they changed. Switching models therefore keeps the context.

## Rate-limit fallback and status

Lumen recognizes rate-limit and quota errors in CLI output, for example `429`, usage limit or `try again in 2h`. It marks that account as cooling down until the parsed reset time, or for 15 minutes if no time is given. The same message then continues on the other provider without being lost, and the activity records the switch. Since 0.4, when the other subscription is cooling down too, the message continues on the emergency providers in order: Antigravity, Gemini CLI, then Ollama. Settings → Teamwork → **Switch on rate limit** turns this off.

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
