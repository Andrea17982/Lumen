import { useEffect, useRef, useState } from 'react';
import { Badge, Button, ConfirmDialog, Menu, Switch, TextField } from '../../design/components';
import { useT } from '../../i18n';
import { useSyncClient, useSyncState } from '../../sync/react';
import { visibleText } from '../attention/requests';
import { usePluginTheme } from './theme';
import s from './Settings.module.css';

interface Server {
  name: string;
  enabled: boolean;
  healthy: boolean;
  target: string;
}
interface Found {
  id: string;
  name: string;
  source: string;
  kind: string;
  status: string;
  reason: string;
}
interface Plugin {
  id: string;
  enabled: boolean;
  error?: string;
  grants?: string[];
  pending?: string[];
  manifest?: { name: string; version: string; description: string; permissions?: { secrets?: string[] } };
}
interface Contributes {
  mcpServers?: Record<string, { command?: string; args?: string[] }>;
  skills?: string[];
  slashCommands?: unknown[];
  panels?: unknown[];
  themes?: unknown[];
}
interface Inspected {
  token: string;
  manifest: { id: string; name: string; version: string; description: string; contributes?: Contributes };
  grants: string[];
  added: string[];
  update: boolean;
  installedVersion: string | null;
}

/** What a plugin adds besides permissions: bundled programs, skills, commands, panels, themes. */
function Brings({ contributes }: { contributes?: Contributes }) {
  const t = useT();
  const servers = Object.entries(contributes?.mcpServers ?? {});
  const items = [
    ...servers.map(([name, server]) => t.extensions.runs(visibleText(name), visibleText([server.command, ...(server.args ?? [])].filter(Boolean).join(' ')))),
    ...(contributes?.skills?.length ? [t.extensions.addsSkills(contributes.skills.length)] : []),
    ...(contributes?.slashCommands?.length ? [t.extensions.addsCommands(contributes.slashCommands.length)] : []),
    ...(contributes?.panels?.length ? [t.extensions.addsPanels(contributes.panels.length)] : []),
    ...(contributes?.themes?.length ? [t.extensions.addsThemes(contributes.themes.length)] : []),
  ];
  if (!items.length) return null;
  return (
    <>
      <p>{t.extensions.brings}</p>
      <ul>
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </>
  );
}

/** Everything agents can use besides themselves: MCP servers, skills and plugins. */
export function Extensions() {
  const t = useT();
  const sync = useSyncClient();
  const [servers, setServers] = useState<Server[]>([]);
  const [status, setStatus] = useState<{ skills: string[]; discovered: Found[] } | null>(null);
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [error, setError] = useState<string | null>(null);
  const fail = (e: unknown) => setError(visibleText(String(e)));
  const load = () =>
    Promise.all([sync.call<Server[]>('mcp_health'), sync.call<{ skills: string[]; discovered: Found[] }>('integration_status'), sync.call<Plugin[]>('plugins_list')]).then(([a, b, c]) => {
      setServers(a);
      setStatus(b);
      setPlugins(c);
    }, fail);
  useEffect(() => {
    void load();
  }, [sync]);
  const run = (command: string, args: Record<string, unknown>) => {
    setError(null);
    return sync.call(command, args).then(
      () => load(),
      (e) => fail(e),
    );
  };

  return (
    <div className={s.section}>
      <p className={s.intro}>{t.extensions.intro}</p>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      <Plugins plugins={plugins} run={run} onError={fail} onChange={() => load().then(() => sync.loadContributions())} />
      <Servers servers={servers} run={run} />
      <section className={s.group} aria-labelledby="skills-title">
        <h3 id="skills-title">{t.extensions.skills}</h3>
        {status && status.skills.length === 0 && <p className={s.quiet}>{t.extensions.skillsNone}</p>}
        {status && status.skills.length > 0 && (
          <ul className={s.rows}>
            {status.skills.map((name) => (
              <li key={name}>
                <span>{visibleText(name)}</span>
              </li>
            ))}
          </ul>
        )}
        <div>
          <Button
            variant="secondary"
            onPress={async () => {
              const path = await sync.pickFolder();
              if (path) await run('install_skill', { path });
            }}
          >
            {t.extensions.addSkill}
          </Button>
        </div>
      </section>
      {status && (
        <section className={s.group} aria-labelledby="found-title">
          <h3 id="found-title">{t.extensions.found}</h3>
          <p className={s.quiet}>{t.extensions.foundHelp}</p>
          {status.discovered.length === 0 ? (
            <p className={s.quiet}>{t.extensions.foundNone}</p>
          ) : (
            <ul className={s.rows}>
              {status.discovered.map((f) =>
                ['available', 'disabled'].includes(f.status) ? (
                  <li key={f.id}>
                    <Switch isSelected={f.status === 'available'} onChange={(on) => void run('toggle_discovered_integration', { id: f.id, on })}>
                      <span className={s.switchText}>
                        {visibleText(f.name)}
                        <small>{visibleText(f.source)}</small>
                      </span>
                    </Switch>
                  </li>
                ) : (
                  <li key={f.id}>
                    <span className={s.switchText}>
                      {visibleText(f.name)}
                      <small>{visibleText(f.reason || f.source)}</small>
                    </span>
                  </li>
                ),
              )}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}

function Servers({ servers, run }: { servers: Server[]; run: (command: string, args: Record<string, unknown>) => Promise<unknown> }) {
  const t = useT();
  const [name, setName] = useState('');
  const [target, setTarget] = useState('');
  const [removing, setRemoving] = useState<string | null>(null);
  return (
    <section className={s.group} aria-labelledby="mcp-title">
      <h3 id="mcp-title">{t.extensions.mcp}</h3>
      {servers.length === 0 && <p className={s.quiet}>{t.extensions.mcpNone}</p>}
      {servers.length > 0 && (
        <ul className={s.rows}>
          {servers.map((server) => (
            <li key={server.name} aria-label={server.name}>
              <Switch isSelected={server.enabled} onChange={(on) => void run('mcp_toggle', { name: server.name, on })}>
                <span className={s.switchText}>
                  {visibleText(server.name)}
                  <small>
                    <code>{visibleText(server.target)}</code>
                  </small>
                </span>
              </Switch>
              <span className={s.spacer} />
              {server.healthy ? <Badge tone="success">{t.extensions.ready}</Badge> : <Badge tone="warning">{t.extensions.notFound}</Badge>}
              <Menu label={t.moreActions} items={[{ id: 'remove', label: t.extensions.remove, danger: true }]} onAction={() => setRemoving(server.name)} />
            </li>
          ))}
        </ul>
      )}
      <form
        className={s.row}
        onSubmit={(e) => {
          e.preventDefault();
          void run('mcp_add', { name: name.trim(), target: target.trim() }).then(() => {
            setName('');
            setTarget('');
          });
        }}
      >
        <TextField label={t.extensions.mcpName} value={name} onChange={setName} />
        <TextField label={t.extensions.mcpTarget} value={target} onChange={setTarget} placeholder="npx -y @modelcontextprotocol/server-memory" />
        <Button variant="secondary" type="submit" isDisabled={!name.trim() || !target.trim()}>
          {t.extensions.mcpAdd}
        </Button>
      </form>
      <ConfirmDialog
        isOpen={removing !== null}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={t.extensions.removeTitle(removing ?? '')}
        body={t.extensions.removeBody}
        confirmLabel={t.extensions.remove}
        cancelLabel={t.cancel}
        danger
        onConfirm={() => void run('mcp_remove', { name: removing })}
      />
    </section>
  );
}

function Plugins({ plugins, run, onError, onChange }: { plugins: Plugin[]; run: (command: string, args: Record<string, unknown>) => Promise<unknown>; onError: (e: unknown) => void; onChange: () => Promise<unknown> }) {
  const t = useT();
  const sync = useSyncClient();
  const [inspected, setInspected] = useState<Inspected | null>(null);
  const [removing, setRemoving] = useState<Plugin | null>(null);
  return (
    <section className={s.group} aria-labelledby="plugins-title">
      <h3 id="plugins-title">{t.extensions.plugins}</h3>
      {plugins.length === 0 && <p className={s.quiet}>{t.extensions.pluginsNone}</p>}
      {plugins.length > 0 && (
        <ul className={s.cards}>
          {plugins.map((p) => (
            <PluginCard key={p.id} plugin={p} run={run} onChange={onChange} onRemove={() => setRemoving(p)} onError={onError} />
          ))}
        </ul>
      )}
      <div>
        <Button
          variant="secondary"
          onPress={async () => {
            const source = await sync.pickFolder();
            // Inspecting copies the folder aside and validates it; nothing in it runs.
            if (source) await sync.call<Inspected>('plugin_inspect', { source }).then(setInspected, onError);
          }}
        >
          {t.extensions.addPlugin}
        </Button>
      </div>
      {inspected && (
        <ConfirmDialog
          isOpen
          onOpenChange={(o) => !o && setInspected(null)}
          title={inspected.update && inspected.installedVersion ? t.extensions.updateTitle(visibleText(inspected.manifest.name), inspected.installedVersion, inspected.manifest.version) : t.extensions.installTitle(visibleText(inspected.manifest.name))}
          body={
            <div className={s.dialogFields}>
              {inspected.manifest.description && <p>{visibleText(inspected.manifest.description)}</p>}
              <Brings contributes={inspected.manifest.contributes} />
              {inspected.grants.length === 0 ? (
                !inspected.manifest.contributes?.mcpServers || Object.keys(inspected.manifest.contributes.mcpServers).length === 0 ? <p>{t.extensions.asksNothing}</p> : null
              ) : (
                <>
                  <p>{inspected.update && inspected.added.length ? t.extensions.asksMore : t.extensions.asks}</p>
                  <ul>
                    {(inspected.update && inspected.added.length ? inspected.added : inspected.grants).map((g) => (
                      <li key={g}>{visibleText(g)}</li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          }
          confirmLabel={t.extensions.install}
          cancelLabel={t.cancel}
          onConfirm={async () => {
            const token = inspected.token;
            setInspected(null);
            // Approving a new plugin here is the consent: it is turned on right away. An update
            // keeps the plugin off if the user had turned it off.
            const wasOff = plugins.some((p) => p.id === inspected.manifest.id && !p.enabled && !p.pending?.length);
            const result = await sync.call<{ id: string; enabled: boolean }>('plugin_install', { token }).catch((e) => {
              onError(e);
              return null;
            });
            if (result && !result.enabled && !wasOff) await sync.call('plugin_set_enabled', { id: result.id, on: true }).catch(onError);
            await onChange();
          }}
        />
      )}
      <ConfirmDialog
        isOpen={removing !== null}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={t.extensions.removeTitle(visibleText(removing?.manifest?.name ?? removing?.id ?? ''))}
        body={t.extensions.removeBody}
        confirmLabel={t.extensions.remove}
        cancelLabel={t.cancel}
        danger
        onConfirm={() => void run('plugin_remove', { id: removing?.id }).then(onChange)}
      />
    </section>
  );
}

function PluginCard({ plugin, run, onChange, onRemove, onError }: { plugin: Plugin; run: (command: string, args: Record<string, unknown>) => Promise<unknown>; onChange: () => Promise<unknown>; onRemove: () => void; onError: (e: unknown) => void }) {
  const t = useT();
  const sync = useSyncClient();
  const contributions = useSyncState((st) => st.contributions);
  const [theme, setTheme] = usePluginTheme();
  const [panel, setPanel] = useState<string | null>(null);
  const [approving, setApproving] = useState(false);
  const name = visibleText(plugin.manifest?.name ?? plugin.id);
  const pending = plugin.pending ?? [];
  const turn = (on: boolean) => void run('plugin_set_enabled', { id: plugin.id, on }).then(onChange);
  const panels = contributions.panels.filter((p) => p.plugin === plugin.id);
  const themes = contributions.themes.filter((x) => x.plugin === plugin.id);
  return (
    <li className={s.card} aria-label={name}>
      <div className={s.cardHead}>
        <div className={s.cardTitle}>
          <strong>{name}</strong>
          {plugin.manifest && <span>{visibleText(plugin.manifest.version)}</span>}
        </div>
        <Switch isSelected={plugin.enabled} isDisabled={!!plugin.error} onChange={(on) => (on && pending.length ? setApproving(true) : turn(on))}>
          {t.extensions.enabled}
        </Switch>
        <Menu
          label={t.moreActions}
          items={[
            { id: 'reload', label: t.extensions.reload },
            { id: 'remove', label: t.extensions.remove, danger: true, separatorBefore: true },
          ]}
          onAction={(id) => (id === 'remove' ? onRemove() : void run('plugin_reload', { id: plugin.id }).then(onChange))}
        />
      </div>
      {plugin.manifest?.description && <p className={s.quiet}>{visibleText(plugin.manifest.description)}</p>}
      {plugin.error && <p className={s.error}>{visibleText(plugin.error)}</p>}
      {plugin.pending && plugin.pending.length > 0 && (
        <p className={s.notice}>
          {t.extensions.pending} {plugin.pending.map(visibleText).join(', ')}
        </p>
      )}
      {plugin.grants && plugin.grants.length > 0 && (
        <ul className={s.grants}>
          {plugin.grants.map((g) => (
            <li key={g}>{visibleText(g)}</li>
          ))}
        </ul>
      )}
      {plugin.enabled && (plugin.manifest?.permissions?.secrets ?? []).map((secret) => <Secret key={secret} plugin={plugin.id} name={secret} onError={onError} />)}
      {themes.map((x) => (
        <Switch key={x.id} isSelected={theme === `${x.plugin}/${x.id}`} onChange={(on) => setTheme(on ? `${x.plugin}/${x.id}` : null)}>
          {t.extensions.useTheme}: {visibleText(x.name)}
        </Switch>
      ))}
      {panels.length > 0 && (
        <div className={s.actions}>
          {panels.map((p) => (
            <Button key={p.id} variant="secondary" onPress={() => setPanel(panel === p.id ? null : p.id)}>
              {panel === p.id ? t.extensions.close : t.extensions.open(visibleText(p.title))}
            </Button>
          ))}
        </div>
      )}
      <ConfirmDialog
        isOpen={approving}
        onOpenChange={setApproving}
        title={t.extensions.approveTitle(name)}
        body={
          <div className={s.dialogFields}>
            <p>{t.extensions.asksMore}</p>
            <ul>
              {pending.map((g) => (
                <li key={g}>{visibleText(g)}</li>
              ))}
            </ul>
          </div>
        }
        confirmLabel={t.extensions.approve}
        cancelLabel={t.cancel}
        onConfirm={() => turn(true)}
      />
      {panel && <Panel plugin={plugin.id} panel={panel} title={visibleText(panels.find((p) => p.id === panel)?.title ?? panel)} src={sync.panelUrl(plugin.id, panel)} />}
    </li>
  );
}

/** A declared secret goes to the keychain; like an API key, it is never read back. */
function Secret({ plugin, name, onError }: { plugin: string; name: string; onError: (e: unknown) => void }) {
  const t = useT();
  const sync = useSyncClient();
  const [value, setValue] = useState('');
  const [saved, setSaved] = useState(false);
  return (
    <form
      className={s.row}
      onSubmit={(e) => {
        e.preventDefault();
        const secret = value;
        setValue('');
        sync.call('plugin_set_secret', { id: plugin, name, value: secret }).then(() => setSaved(true), onError);
      }}
    >
      <TextField label={t.extensions.secret(visibleText(name))} type="password" autoComplete="off" value={value} onChange={setValue} description={saved ? t.extensions.secretSaved : undefined} />
      <Button variant="secondary" type="submit" isDisabled={!value}>
        {t.extensions.saveSecret}
      </Button>
    </form>
  );
}

const MAX_INSERT = 20_000;
const MAX_IN_FLIGHT = 4;

/**
 * A plugin panel in a sandboxed frame with an opaque origin. Its only way out is a message to this
 * page, which answers two methods: `prompt.insert` and `net.fetch` (through the daemon's host
 * allowlist). Messages from anything but this frame are ignored.
 */
function Panel({ plugin, panel, title, src }: { plugin: string; panel: string; title: string; src: string | null }) {
  const t = useT();
  const sync = useSyncClient();
  const frame = useRef<HTMLIFrameElement>(null);
  const [note, setNote] = useState<string | null>(null);
  useEffect(() => {
    let inFlight = 0;
    const onMessage = (e: MessageEvent) => {
      const target = frame.current?.contentWindow;
      if (!target || e.source !== target) return;
      const m = e.data as { lumen?: unknown; id?: unknown; method?: unknown; params?: { text?: unknown; url?: unknown } };
      if (!m || m.lumen !== 1 || (typeof m.id !== 'number' && typeof m.id !== 'string')) return;
      // The frame's origin is opaque ("null"), so a reply can only be addressed with "*".
      const reply = (body: { result?: unknown; error?: string }) => target.postMessage({ lumen: 1, id: m.id, ...body }, '*');
      if (m.method === 'prompt.insert' && typeof m.params?.text === 'string' && m.params.text.length <= MAX_INSERT) {
        // Hidden characters are shown escaped, so the user reads exactly what an agent would get.
        if (!sync.insertPrompt(visibleText(m.params.text))) return reply({ error: 'The message box is full' });
        setNote(t.extensions.inserted);
        return reply({ result: true });
      }
      if (m.method === 'net.fetch' && typeof m.params?.url === 'string') {
        if (inFlight >= MAX_IN_FLIGHT) return reply({ error: 'Too many requests' });
        inFlight++;
        void sync
          .call('plugin_panel_fetch', { id: plugin, url: m.params.url })
          .then(
            (result) => reply({ result }),
            (error) => reply({ error: String(error) }),
          )
          .finally(() => inFlight--);
        return;
      }
      reply({ error: 'Unknown method' });
    };
    addEventListener('message', onMessage);
    return () => removeEventListener('message', onMessage);
  }, [sync, plugin, panel, t]);
  if (!src) return null;
  return (
    <div className={s.panel}>
      <iframe ref={frame} title={title} src={src} sandbox="allow-scripts" referrerPolicy="no-referrer" />
      {note && (
        <p className={s.quiet} role="status">
          {note}
        </p>
      )}
    </div>
  );
}
