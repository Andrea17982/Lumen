# Godot plugin

Godot support for Lumen as a separate plugin. It replaces the Godot controls that used to be built into the project inspector.

- **`godot_info`** finds the Godot projects in the conversation folder, the Godot editor and the Godot MCP servers the project declares (names only).
- **`godot_open_editor`** opens a project in the Godot editor.
- **Skill `godot`** tells agents how to run, stop, reload and capture scenes through the project's own Godot MCP server, and to check the running game before saying a change works.
- **Slash commands** `/godot-play` and `/godot-editor`.

Running and capturing the game goes through the Godot MCP server your project declares in `.mcp.json` or `.codex/config.toml`, which every assistant already receives as a project MCP server. This plugin never starts that server.

## Install

**Settings → Plugins → Choose folder**, then pick this folder. Needs Node.js.

## Permissions

- `fsProject: read`, to find `project.godot` files and read the MCP declarations.
- `shell: true`, because opening the editor starts the Godot executable.
