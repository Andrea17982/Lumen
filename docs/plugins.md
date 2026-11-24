# Plugins

A Lumen plugin is a folder with a `lumen-plugin.json` manifest. It can add themes, slash commands, skills, MCP tools and small panels. Each plugin declares up front everything it may touch. You see that list before you install, the plugin stays off until you turn it on, and Lumen checks it again every time the plugin is used.

Three examples ship in [plugins/examples](../plugins/examples/README.md).

## Install, enable, update

Open **Settings → Plugins**.

1. Choose a folder, or paste an `https://` git URL. Lumen clones it with your `git`.
2. **Review** shows the name, the version and what the plugin asks to do, for example "Connect to api.example.com". Nothing runs during the review.
3. **Install (off)** copies the plugin into Lumen's data folder. A new plugin is always off.
4. Turning the switch on shows the permissions once more. Turning it on approves exactly those permissions.
5. **Reload** reads the plugin again from where you installed it, so you can edit a plugin without restarting Lumen. If the new version asks for nothing new, it applies at once. If it asks for more, Lumen shows only the new permissions, and the plugin stays off until you turn it on again.
6. **Remove** deletes the copy, its state and its stored secrets.

A plugin whose files change on disk to ask for more than you approved stops working until you approve it again.

## Manifest

```json
{
  "id": "word-count",
  "name": "Word count",
  "version": "1.0.0",
  "description": "Counts words in text.",
  "lumen": ">=0.10.0 <1.0.0",
  "contributes": {
    "themes": [{ "id": "dusk", "name": "Dusk", "file": "dusk.css" }],
    "slashCommands": [{ "name": "review", "description": "Review changes", "prompt": "Review…" }],
    "skills": ["skills/careful-review"],
    "mcpServers": { "words": { "command": "node", "args": ["server.mjs"] } },
    "panels": [{ "id": "picker", "title": "Picker", "entry": "picker.html" }]
  },
  "permissions": {
    "fsProject": "none",
    "network": [],
    "shell": false,
    "secrets": []
  }
}
```

- `id` is 2–64 lowercase letters, digits or dashes. `version` looks like `1.2.3`.
- `lumen` is a list of comparators that all must match the running Lumen version, for example `>=0.12.0 <0.13.0`.
- Unknown fields are rejected. Every path must stay inside the plugin folder, and symbolic links are refused.

### Permissions

All four permissions default to none.

| Permission | Values | Effect |
| --- | --- | --- |
| `fsProject` | `none`, `read`, `write` | The MCP server receives `LUMEN_PROJECT_DIR` and `LUMEN_PROJECT_ACCESS`. Without it, the server never learns the project path. |
| `network` | host names, `*.example.com` | Panels may fetch these hosts over HTTPS through Lumen. MCP servers receive the list in `LUMEN_NETWORK_HOSTS`. |
| `shell` | `true` / `false` | Required for an MCP command that is not a file shipped in the plugin. |
| `secrets` | `UPPER_SNAKE_CASE` names | You enter values in Settings. Lumen stores them in the OS keychain and passes only the declared names to the plugin's MCP server. |

## Contributions

**Themes** are CSS files that only set custom properties (`--accent: #b5562f;`) under `:root`, `:root[data-theme='light']` or `:root[data-theme='dark']`. Selectors, `url()`, `@import` and anything that is not a token are rejected, so a theme cannot run code or load anything. Pick a theme under its plugin in Settings.

**Slash commands** add entries to the `/` menu. Choosing one puts its prompt in the composer, where you can still edit it before you send. Commands never run on their own, and the names `workspace` and `compact` are reserved.

**Skills** are folders with a `SKILL.md`. Enabled plugin skills join the [shared skills](providers.md#shared-integrations), so every assistant, including Auto, can list them and read them.

**MCP servers** join the shared MCP list, so Claude, GPT, Antigravity, Ollama and Auto can use them under the conversation's normal permission mode. Each server starts through `lumen-daemon --lumen-plugin-mcp`. Before launching, the daemon re-checks that the plugin is enabled and still matches what you approved, then:

- runs the program in the plugin folder;
- clears the environment, keeping only the basics a process needs to start (`PATH`, `HOME`, temp folders and the system root), so your API keys and tokens are not passed on;
- adds the declared secrets and, only with `fsProject`, the project path;
- refuses to start a command outside the plugin unless `shell` is declared. Without `shell`, the only things that can run are a file in the plugin, or `node`, `python`, `python3`, `deno` or `bun` running a script in the plugin.

**Panels** are one HTML file each, opened from Settings in an iframe with `sandbox="allow-scripts"` and served by a separate `lumen-plugin:` protocol with its own policy: `default-src 'none'`, inline script and style only, and no `connect-src`. The panel has an opaque origin. It cannot read Lumen's page, storage or cookies, cannot call Tauri IPC, and cannot open network connections itself. Its only API is `postMessage` to the parent:

```js
parent.postMessage({ lumen: 1, id: 1, method: 'prompt.insert', params: { text: 'Hi' } }, '*');
parent.postMessage(
  { lumen: 1, id: 2, method: 'net.fetch', params: { url: 'https://api.example.com/x' } },
  '*',
);
// Replies arrive as { lumen: 1, id, result } or { lumen: 1, id, error }.
```

`prompt.insert` puts text in the composer. `net.fetch` makes a GET request to a declared HTTPS host and returns the status, the content type and up to 1 MB of the body as text. Any other method is refused.

## What is enforced, and what is not

Lumen enforces the following in its own code, and tests cover each item: manifest validation, theme filtering, the panel sandbox and broker, the HTTPS host allowlist for panels, the MCP launch rules, environment scrubbing and secret scoping, and re-approval when permissions grow.

An MCP server is still a normal program on your computer. Lumen controls what that program is given, but it does not put the process in an operating-system sandbox. So `fsProject` and `network` tell the server what it is allowed to do, but nothing stops a server that ignores them. Its tool calls still go through the conversation's approval mode. Install MCP plugins only from authors you trust, and prefer plugins that declare `shell: false`.

The trade-off: a v1 plugin cannot add new screens or change how Lumen itself behaves.
