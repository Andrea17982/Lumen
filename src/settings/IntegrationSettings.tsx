import { useCallback, useEffect, useState } from 'react';
import { Plus, Trash2 } from 'lucide-react';
import { call, desktop } from '../lib/bridge';
import type { McpServer } from '../lib/types';
import { useI18n } from '../lib/i18n';
interface Inventory {
  path: string;
  skills: string[];
  mcp: string;
}

/** One MCP hub and skill library shared by Claude and ChatGPT. */
export default function IntegrationSettings({ onError }: { onError: (s: string) => void }) {
  const { t } = useI18n();
  const [inventory, setInventory] = useState<Inventory | null>(null);
  const [servers, setServers] = useState<McpServer[]>([]);
  const [json, setJson] = useState('');
  const [busy, setBusy] = useState(false);
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState('');
  const [target, setTarget] = useState('');
  const refresh = useCallback(async () => {
    const data = await call<Inventory>('integration_status');
    setInventory(data);
    setJson(data.mcp);
    setServers(await call<McpServer[]>('mcp_health'));
  }, []);
  useEffect(() => {
    if (desktop) void refresh().catch((e) => onError(String(e)));
  }, [onError, refresh]);
  async function act(fn: () => Promise<unknown>) {
    setBusy(true);
    try {
      await fn();
      await refresh();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="settings-section">
      <div className="section-title">
        <h3>{t('Tools (MCP)')}</h3>
        <button
          className="icon-button"
          aria-label={t('Add MCP server')}
          disabled={!desktop}
          onClick={() => setAdding(!adding)}
        >
          <Plus size={17} />
        </button>
      </div>
      <p>{t('Turn a tool on once and both assistants can use it when edits are allowed.')}</p>
      {adding && (
        <form
          className="account-form"
          onSubmit={(e) => {
            e.preventDefault();
            void act(async () => {
              await call('mcp_add', { name, target });
              setName('');
              setTarget('');
              setAdding(false);
            });
          }}
        >
          <label>
            {t('Name')}
            <input
              required
              autoFocus
              pattern="[A-Za-z0-9_\-]+"
              placeholder="sqlite"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label>
            {t('Command or URL')}
            <input
              required
              placeholder="npx -y @modelcontextprotocol/server-filesystem ."
              value={target}
              onChange={(e) => setTarget(e.target.value)}
            />
          </label>
          <button className="primary-button" disabled={busy}>
            {t('Add tool')}
          </button>
        </form>
      )}
      {servers.length === 0 && !adding && <div className="account-empty">{t('No tools yet.')}</div>}
      {servers.map((s) => (
        <div className="mcp-row" key={s.name}>
          <span
            className={`health-dot ${s.healthy ? 'active' : 'cooldown'}`}
            title={s.healthy ? t('Ready') : t('Command not found')}
          />
          <div className="account-info">
            <strong>{s.name}</strong>
            <small title={s.target}>{s.healthy ? s.target : `Not found: ${s.target}`}</small>
          </div>
          <input
            type="checkbox"
            role="switch"
            className="switch"
            aria-label={`${t('Enable')} ${s.name}`}
            checked={s.enabled}
            disabled={busy}
            onChange={(e) =>
              void act(() => call('mcp_toggle', { name: s.name, on: e.target.checked }))
            }
          />
          <button
            className="icon-button"
            aria-label={`${t('Remove')} ${s.name}`}
            disabled={busy}
            onClick={() => void act(() => call('mcp_remove', { name: s.name }))}
          >
            <Trash2 size={14} />
          </button>
        </div>
      ))}
      <div className="panel-row">
        <small>
          {t('Skills')}:{' '}
          {inventory?.skills.length ? inventory.skills.join(', ') : t('none installed')}
        </small>
        <button
          className="text-button"
          disabled={!desktop || busy}
          onClick={() =>
            void act(async () => {
              const { open } = await import('@tauri-apps/plugin-dialog');
              const path = await open({
                directory: true,
                multiple: false,
                title: t('Import skill folder containing SKILL.md'),
              });
              if (typeof path === 'string') await call('install_skill', { path });
            })
          }
        >
          {t('Import skill')}
        </button>
      </div>
      <details className="raw-config">
        <summary>{t('Edit as JSON')}</summary>
        <textarea
          className="integration-editor"
          aria-label={t('Shared MCP configuration')}
          value={json}
          onChange={(e) => setJson(e.target.value)}
          spellCheck={false}
        />
        <button
          className="primary-button"
          disabled={!desktop || busy}
          onClick={() => void act(() => call('save_mcp', { json }))}
        >
          {t('Save')}
        </button>
      </details>
    </section>
  );
}
