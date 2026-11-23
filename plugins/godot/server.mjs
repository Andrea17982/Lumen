// Godot plugin MCP server, no dependencies. It reads the project folder (fsProject: read) to find
// Godot projects and Godot-named MCP declarations, and starts the Godot editor on request
// (shell: true). It never starts the project's own MCP servers and never returns their arguments
// or environment, which can hold tokens.
import { spawn } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, join, relative, resolve, sep } from 'node:path';
import { createInterface } from 'node:readline';

const root = process.env.LUMEN_PROJECT_DIR ? resolve(process.env.LUMEN_PROJECT_DIR) : '';
const windows = process.platform === 'win32';
const SKIP = new Set(['.git', '.godot', '.import', 'node_modules', 'target', 'build', 'dist']);

const send = (frame) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...frame }) + '\n');
const text = (value, isError = false) => ({
  content: [{ type: 'text', text: typeof value === 'string' ? value : JSON.stringify(value, null, 2) }],
  isError,
});

/** `project.godot` folders up to three levels deep, relative to the project. */
function projects(dir = root, depth = 0, out = []) {
  if (!dir) return out;
  if (existsSync(join(dir, 'project.godot'))) out.push(relative(root, dir) || '.');
  if (depth >= 3) return out;
  let entries = [];
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const entry of entries)
    if (entry.isDirectory() && !SKIP.has(entry.name) && !entry.name.startsWith('.'))
      projects(join(dir, entry.name), depth + 1, out);
  return out.sort();
}

/** Godot executables on PATH, then the usual per-user install folders; newest name last wins. */
function executable() {
  const names = windows
    ? ['godot.exe', 'godot4.exe', 'godot-mono.exe', 'godot4-mono.exe']
    : ['godot', 'godot4', 'godot-mono', 'godot4-mono'];
  for (const dir of (process.env.PATH || '').split(delimiter))
    for (const name of names) if (dir && existsSync(join(dir, name))) return join(dir, name);
  const found = [];
  const collect = (dir, depth) => {
    let entries = [];
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const name = entry.name.toLowerCase();
      const path = join(dir, entry.name);
      if (name.startsWith('godot') && !name.includes('console') && /\.(exe|app)$/.test(name))
        found.push(path);
      else if (entry.isDirectory() && depth < 3) collect(path, depth + 1);
    }
  };
  collect(join(homedir(), 'AppData', 'Local', 'Programs', 'Godot'), 0);
  if (process.platform === 'darwin') collect('/Applications', 0);
  return found.sort().at(-1) || null;
}

/** Names of Godot-named servers in the project's MCP declarations; never their commands. */
function declaredServers() {
  const names = new Set();
  try {
    const json = JSON.parse(readFileSync(join(root, '.mcp.json'), 'utf8'));
    for (const name of Object.keys(json.mcpServers || {}))
      if (name.toLowerCase().includes('godot')) names.add(`${name} (.mcp.json)`);
  } catch {}
  try {
    const toml = readFileSync(join(root, '.codex', 'config.toml'), 'utf8');
    for (const [, name] of toml.matchAll(/^\[mcp_servers\.([^\]]+)\]/gm))
      if (name.toLowerCase().includes('godot')) names.add(`${name} (.codex/config.toml)`);
  } catch {}
  return [...names];
}

const tools = [
  {
    name: 'godot_info',
    description:
      "Godot projects in this folder (paths of folders with project.godot), the Godot editor executable if installed, and the Godot MCP servers the project declares. Reads files only; starts nothing.",
    inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    annotations: { readOnlyHint: true },
  },
  {
    name: 'godot_open_editor',
    description:
      'Open a Godot project from this folder in the Godot editor. `project` is a folder from godot_info (default: the only project). `executable` overrides the detected editor path.',
    inputSchema: {
      type: 'object',
      properties: { project: { type: 'string' }, executable: { type: 'string' } },
      additionalProperties: false,
    },
    annotations: { readOnlyHint: false, destructiveHint: false },
  },
];

function call(name, args = {}) {
  if (!root) return text('This plugin needs project read access (fsProject) to find Godot projects.', true);
  if (name === 'godot_info')
    return text({
      projects: projects(),
      executable: executable(),
      mcpServers: declaredServers(),
      note: 'Run, stop, reload and capture scenes with the tools of the declared Godot MCP server (for example play_scene, stop_scene, reload_project, get_game_screenshot).',
    });
  if (name === 'godot_open_editor') {
    const found = projects();
    const project = args.project ?? (found.length === 1 ? found[0] : undefined);
    if (project === undefined)
      return text(
        found.length ? `Choose one project: ${found.join(', ')}` : 'No project.godot found in this folder.',
        true,
      );
    const dir = resolve(root, project);
    if ((dir !== root && !dir.startsWith(root + sep)) || !existsSync(join(dir, 'project.godot')))
      return text('Choose a folder with project.godot inside this project.', true);
    let program = args.executable || executable();
    // A macOS bundle is a folder; its binary is inside.
    if (program?.endsWith('.app')) program = join(program, 'Contents', 'MacOS', 'Godot');
    if (!program || !existsSync(program) || !statSync(program).isFile())
      return text('Godot editor not found. Install Godot or pass `executable`.', true);
    const child = spawn(program, ['--editor', '--path', dir], { detached: true, stdio: 'ignore' });
    child.on('error', () => {});
    child.unref();
    return text(`Opened ${project} in ${program}.`);
  }
  return text(`Unknown tool ${name}`, true);
}

createInterface({ input: process.stdin }).on('line', (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return;
  }
  const { id, method, params } = message;
  if (id === undefined) return;
  if (method === 'initialize')
    send({
      id,
      result: {
        protocolVersion: params?.protocolVersion || '2025-06-18',
        capabilities: { tools: {} },
        serverInfo: { name: 'godot', version: '1.0.0' },
      },
    });
  else if (method === 'tools/list') send({ id, result: { tools } });
  else if (method === 'tools/call') send({ id, result: call(params?.name, params?.arguments) });
  else send({ id, error: { code: -32601, message: `Unknown method ${method}` } });
});
