import { useCallback, useEffect, useState } from 'react';
import { Play, PlugZap, RefreshCw, SquareArrowOutUpRight } from 'lucide-react';
import { call } from '../lib/bridge';
import { errorText, IconButton, nameOf } from '../lib/util';
import type {
  ChangeLayer,
  CheckResult,
  GodotConnection,
  GodotDiagnostics,
  ProjectCheck,
  ProjectTool,
} from '../lib/types';

/** Project-local skills and MCP declarations (read-only) and checks that run only on request. */
export default function ProjectTools({
  folder,
  onError,
}: {
  folder: string;
  onError: (s: string) => void;
}) {
  const [tools, setTools] = useState<ProjectTool[]>([]);
  const [checks, setChecks] = useState<ProjectCheck[]>([]);
  const [godot, setGodot] = useState<GodotDiagnostics | null>(null);
  const [running, setRunning] = useState('');
  const [results, setResults] = useState<Record<string, CheckResult>>({});
  const [connection, setConnection] = useState<GodotConnection | null>(null);
  const [layers, setLayers] = useState<ChangeLayer[] | null>(null);
  const loadLayers = useCallback(
    () =>
      call<ChangeLayer[]>('change_layers', { folder })
        .then(setLayers)
        .catch(() => setLayers(null)),
    [folder],
  );
  useEffect(() => {
    let gone = false;
    setResults({});
    setConnection(null);
    void loadLayers();
    void Promise.all([
      call<ProjectTool[]>('project_tools', { folder }),
      call<ProjectCheck[]>('project_checks', { folder }),
      call<GodotDiagnostics>('project_godot_diagnostics', { folder }),
    ])
      .then(([t, c, g]) => {
        if (gone) return;
        setTools(t);
        setChecks(c);
        setGodot(g);
      })
      .catch((e) => onError(errorText(e)));
    return () => {
      gone = true;
    };
  }, [folder, onError, loadLayers]);

  async function run(check: ProjectCheck) {
    setRunning(check.id);
    try {
      const result = await call<CheckResult>('run_project_check', {
        folder,
        id: check.id,
        command: check.command,
        cwd: check.cwd,
      });
      setResults((old) => ({ ...old, [check.id]: result }));
    } catch (e) {
      onError(errorText(e));
    } finally {
      setRunning('');
    }
  }

  async function testGodot(server: string) {
    setRunning(`godot:${server}`);
    try {
      setConnection(await call<GodotConnection>('godot_connection_test', { folder, server }));
    } catch (e) {
      onError(errorText(e));
    } finally {
      setRunning('');
    }
  }

  async function openEditor(project: string) {
    try {
      await call('open_godot_editor', { folder, project });
    } catch (e) {
      onError(errorText(e));
    }
  }

  async function open(file: string, line: number) {
    try {
      await call<boolean>('open_source_location', { folder, file, line });
    } catch (e) {
      onError(errorText(e));
    }
  }

  return (
    <>
      {godot && (godot.projects.length > 0 || godot.mcpServers.length > 0 || godot.executable) && (
        <section className="tools-section" aria-label="Godot diagnostics">
          <div className="panel-label">
            Godot <small>Project-scoped, read-only diagnostics</small>
          </div>
          <div className="tool-row">
            <strong>
              {godot.projects.length
                ? `${godot.projects.length} project file(s)`
                : 'No project.godot found'}
            </strong>
            {godot.projects.map((project) => (
              <div key={project} className="panel-row">
                <small>{project}</small>
                {godot.executable && (
                  <button
                    className="text-button"
                    aria-label={`Open ${project} in the Godot editor`}
                    onClick={() => void openEditor(project)}
                  >
                    <SquareArrowOutUpRight size={12} /> Open editor
                  </button>
                )}
              </div>
            ))}
            <small>
              {godot.executable
                ? `Executable: ${godot.executable}`
                : 'Godot executable not found on PATH or in the usual install folders'}
            </small>
            <small>
              {godot.mcpServers.length
                ? `Declared MCP: ${godot.mcpServers.join(', ')}`
                : 'No Godot-named MCP declaration found'}
            </small>
            {godot.servers.map((server) => (
              <div key={server} className="panel-row">
                <small>{server}</small>
                <button
                  className="text-button"
                  disabled={!!running}
                  title="Starts this declared server, asks the editor for its project info (read-only) and stops the server"
                  onClick={() => void testGodot(server)}
                >
                  <PlugZap size={12} />{' '}
                  {running === `godot:${server}` ? 'Testing…' : 'Test connection'}
                </button>
              </div>
            ))}
            {connection ? (
              <>
                <small
                  role="status"
                  className={connection.editorConnected ? 'check-ok' : 'tool-conflict'}
                >
                  {connection.editorConnected
                    ? `Editor connected · ${connection.serverInfo} · ${connection.toolCount} tools`
                    : connection.started
                      ? `Server ${connection.serverInfo} started (${connection.toolCount} tools); editor not connected`
                      : 'Server did not start'}
                </small>
                <pre className="check-output">{connection.detail}</pre>
              </>
            ) : (
              <small>{godot.editorConnection}</small>
            )}
          </div>
        </section>
      )}
      {layers && (
        <section className="tools-section" aria-label="Change review">
          <div className="panel-row">
            <div className="panel-label">
              Change review <small>Uncommitted files by layer</small>
            </div>
            <IconButton label="Refresh change review" onClick={() => void loadLayers()}>
              <RefreshCw size={12} />
            </IconButton>
          </div>
          {layers.length === 0 && <p className="panel-hint">No uncommitted changes.</p>}
          {layers.map((layer) => (
            <div key={layer.name} className="tool-row">
              <strong>
                {layer.name} <small>{layer.files.length} file(s)</small>
              </strong>
              {layer.files.slice(0, 12).map((f) => (
                <button
                  key={f.path}
                  className="text-button"
                  disabled={f.status === 'D'}
                  onClick={() => void open(f.path, 1)}
                >
                  <small>
                    {f.status} {f.path}
                  </small>
                </button>
              ))}
              {layer.files.length > 12 && <small>and {layer.files.length - 12} more</small>}
              {layer.checks.length === 0 ? (
                <small>No project check covers this layer.</small>
              ) : (
                layer.checks
                  .map((id) => checks.find((c) => c.id === id))
                  .filter((c): c is ProjectCheck => !!c)
                  .map((c) => (
                    <div key={c.id} className="panel-row">
                      <small>
                        {c.label}
                        {results[c.id] && (results[c.id].ok ? ' · passed' : ' · failed')}
                      </small>
                      <button
                        className="text-button"
                        disabled={!!running}
                        aria-label={`Run ${c.label}`}
                        onClick={() => void run(c)}
                      >
                        <Play size={12} /> {running === c.id ? 'Running…' : 'Run'}
                      </button>
                    </div>
                  ))
              )}
            </div>
          ))}
        </section>
      )}
      <section className="tools-section">
        <div className="panel-label">
          Project tools <small>Declared in this folder · nothing is started</small>
        </div>
        {tools.length === 0 && (
          <p className="panel-hint">No project skills or MCP servers found.</p>
        )}
        {tools.map((t) => (
          <div key={`${t.kind}:${t.origin}:${t.name}`} className="tool-row" title={t.trust}>
            <strong>{t.name}</strong>
            <small>
              {t.kind === 'skill' ? 'Skill' : 'MCP'} · {nameOf(t.provider)} · {t.origin}
            </small>
            {t.detail && <small>{t.detail}</small>}
            {t.conflict && (
              <small className="tool-conflict">
                Also declared elsewhere with a different target; neither is overwritten.
              </small>
            )}
          </div>
        ))}
      </section>
      <section className="tools-section">
        <div className="panel-label">
          Project checks <small>Run only when you press Run</small>
        </div>
        {checks.length === 0 && (
          <p className="panel-hint">
            No checks found. Add package.json scripts or a .lumen/checks.json list.
          </p>
        )}
        {checks.map((c) => {
          const result = results[c.id];
          return (
            <div key={c.id} className="tool-row">
              <div className="panel-row">
                <strong>{c.label}</strong>
                <button
                  className="text-button"
                  disabled={!!running}
                  aria-label={`Run ${c.label}`}
                  onClick={() => void run(c)}
                >
                  <Play size={12} /> {running === c.id ? 'Running…' : 'Run'}
                </button>
              </div>
              <small>
                {c.cwd === '.' ? 'project root' : c.cwd} · {c.source} ·{' '}
                {c.playtest ? 'Needs a playtest' : 'Headless'}
              </small>
              <small>
                <code>{c.command}</code>
              </small>
              {result && (
                <>
                  <small role="status" className={result.ok ? 'check-ok' : 'tool-conflict'}>
                    {result.ok ? 'Passed' : `Failed (exit ${result.exitCode ?? 'signal'})`} ·{' '}
                    {(result.millis / 1000).toFixed(1)}s
                    {result.truncated ? ' · output shortened to the last lines' : ''}
                  </small>
                  {result.links.map((l) => (
                    <button
                      key={`${l.file}:${l.line}`}
                      className="text-button"
                      aria-label={`Open ${l.file} at line ${l.line}`}
                      title="Opens at this line in VS Code, Cursor, Zed or Sublime Text when installed; otherwise in the default application"
                      onClick={() => void open(l.file, l.line)}
                    >
                      <small>
                        Source: {l.file}:{l.line}
                      </small>
                    </button>
                  ))}
                  <pre className="check-output">{result.output || '(no output)'}</pre>
                </>
              )}
            </div>
          );
        })}
      </section>
    </>
  );
}
