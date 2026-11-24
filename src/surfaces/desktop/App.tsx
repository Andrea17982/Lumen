import { useEffect, useState } from 'react';
import { Badge, Button, IconButton, Menu, type Tone } from '../../design/components';
import { useAppearance } from '../../design/appearance';
import { I18nProvider, systemLocale, useT, type Locale } from '../../i18n';
import { Conversation } from '../../features/conversation/Conversation';
import { ProjectPanel } from '../../features/sessions/ProjectPanel';
import { currentProject } from '../../features/sessions/sessions';
import { Sidebar } from '../../features/sessions/Sidebar';
import { Settings } from '../../features/settings/Settings';
import type { Connection, Session } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import s from './App.module.css';

const tones: Record<string, Tone> = { running: 'accent', queued: 'neutral', waiting: 'warning', interrupted: 'warning', partial: 'warning', completed: 'success', failed: 'danger', error: 'danger' };

/**
 * Desktop surface: the daemon's sessions and the selected conversation, kept in sync across
 * reconnections.
 */
export function App() {
  useAppearance();
  const query = new URLSearchParams(location.search).get('locale');
  const locale: Locale = query === 'it' || query === 'en' ? query : systemLocale();
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);
  return (
    <I18nProvider locale={locale}>
      <Shell />
    </I18nProvider>
  );
}

// Per-viewer conveniences only: the daemon owns everything that matters.
function remembered(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function remember(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Storage can be unavailable; the choice then lasts for this window only.
  }
}

function Shell() {
  const t = useT();
  const sync = useSyncClient();
  const sessions = useSyncState((st) => st.sessions);
  const workspaces = useSyncState((st) => st.workspaces);
  const [selected, setSelected] = useState<string | null>(null);
  const [projectId, setProjectId] = useState<string | null>(() => remembered('lumen.project'));
  const [hidden, setHidden] = useState(() => remembered('lumen.sidebar') === 'hidden');
  const [details, setDetails] = useState(false);
  const [settings, setSettings] = useState(false);
  useEffect(() => {
    if (!selected) return;
    sync.watch(selected);
    return () => sync.unwatch(selected);
  }, [sync, selected]);
  const session = sessions.find((x) => x.id === selected);
  const project = currentProject(workspaces, sessions, projectId);
  // Once chosen, the open project stays put: moving or updating a conversation never switches it.
  useEffect(() => {
    if (project && project.id !== projectId) setProjectId(project.id);
  }, [project, projectId]);
  const connected = useSyncState((st) => st.connection.state === 'connected');
  const [error, setError] = useState<string | null>(null);
  const busy = !!session && ['running', 'queued', 'waiting'].includes(session.status);

  const hide = (value: boolean) => {
    setHidden(value);
    remember('lumen.sidebar', value ? 'hidden' : null);
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        hide(!hidden);
      }
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  });

  function choose(id: string | null) {
    setSettings(false);
    setSelected(id);
  }

  function openProject(id: string | null) {
    setProjectId(id);
    remember('lumen.project', id);
    if (id !== project?.id) setSelected(null);
  }

  // Handoff: the daemon writes a new conversation seeded with this one's goal and changes.
  async function compact(id: string) {
    setError(null);
    try {
      const next = await sync.mutate<Session>('compact_session', { id });
      setSelected(next.id);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className={s.window} data-sidebar={hidden ? 'hidden' : undefined}>
      {!hidden && <Sidebar project={project} selected={selected} onSelect={choose} onProject={openProject} onHide={() => hide(true)} onSettings={() => setSettings(!settings)} footer={<ConnectionStatus />} />}
      <main className={s.main}>
        <Banners />
        {settings ? (
          <Settings onClose={() => setSettings(false)} leading={hidden && <IconButton icon="sidebar" label={t.sidebar.show} onPress={() => hide(false)} />} />
        ) : session ? (
          <>
            <header className={s.header}>
              {hidden && <IconButton icon="sidebar" label={t.sidebar.show} onPress={() => hide(false)} />}
              <h1>{session.title || t.untitled}</h1>
              <StatusBadge status={session.status} />
              <span className={s.spacer} />
              <IconButton icon="folder" label={t.project.open} onPress={() => setDetails(!details)} />
              <Menu label={t.moreActions} items={[{ id: 'compact', label: t.recovery.compact, isDisabled: !connected || busy }]} onAction={() => void compact(session.id)} />
            </header>
            {error && (
              <p className={s.headerError} role="alert">
                {error}
              </p>
            )}
            <div className={s.body}>
              <div className={s.column}>
                <Conversation key={session.id} session={session} />
              </div>
              {details && <ProjectPanel key={session.id} session={session} onClose={() => setDetails(false)} />}
            </div>
          </>
        ) : (
          <>
            {hidden && (
              <header className={s.header}>
                <IconButton icon="sidebar" label={t.sidebar.show} onPress={() => hide(false)} />
              </header>
            )}
            <p className={s.empty}>{project ? t.chooseSession : t.sidebar.noProjects}</p>
          </>
        )}
      </main>
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const t = useT();
  return <Badge tone={tones[status] ?? 'neutral'}>{t.status[status] ?? status}</Badge>;
}

function ConnectionStatus() {
  const t = useT();
  const connection = useSyncState((st) => st.connection);
  return (
    <p className={s.connection} role="status" data-state={connection.state}>
      <span className={s.dot} aria-hidden="true" />
      {t.connection[connection.state]}
    </p>
  );
}

function Banners() {
  const t = useT();
  const sync = useSyncClient();
  const connection = useSyncState((st) => st.connection);
  const stopping = useSyncState((st) => st.stopping);
  const unconfirmed = useSyncState((st) => st.unconfirmed);
  const synced = useSyncState((st) => st.synced);
  const sessions = useSyncState((st) => st.sessions);
  return (
    <div className={s.banners}>
      <ConnectionBanner connection={connection} stale={!synced && sessions.length > 0} />
      {stopping && <p className={s.banner} data-tone="warning">{t.banner.stopping}</p>}
      {unconfirmed.map((u) => (
        <div key={u.id} className={s.banner} data-tone="warning" role="alert">
          <span>{t.banner.unconfirmed}</span>
          <Button variant="secondary" onPress={() => sync.acknowledge(u.id)}>
            {t.banner.checked}
          </Button>
        </div>
      ))}
    </div>
  );
}

function ConnectionBanner({ connection, stale }: { connection: Connection; stale: boolean }) {
  const t = useT();
  if (connection.state === 'mismatch') return <p className={s.banner} data-tone="warning">{t.banner.mismatch}</p>;
  if (connection.state === 'failed')
    return (
      <p className={s.banner} data-tone="danger" title={connection.detail}>
        {t.banner.failed}
      </p>
    );
  if (connection.state === 'reconnecting' || (connection.state === 'connected' && stale)) return <p className={s.banner}>{t.banner.stale}</p>;
  return null;
}
