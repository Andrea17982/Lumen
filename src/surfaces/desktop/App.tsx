import { useEffect, useState } from 'react';
import { Mark, Wordmark } from '../../brand/Logo';
import { Badge, Button, type Tone } from '../../design/components';
import { useAppearance } from '../../design/appearance';
import { I18nProvider, systemLocale, useT, type Locale } from '../../i18n';
import type { Connection, Message } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import s from './App.module.css';

const tones: Record<string, Tone> = { running: 'accent', queued: 'neutral', waiting: 'warning', interrupted: 'warning', partial: 'warning', completed: 'success', failed: 'danger', error: 'danger' };

/**
 * M2 desktop surface: the daemon's sessions and the selected transcript, kept in sync across
 * reconnections. The conversation view and composer arrive in M3.
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

function Shell() {
  const t = useT();
  const sync = useSyncClient();
  const sessions = useSyncState((st) => st.sessions);
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(() => {
    if (!selected) return;
    sync.watch(selected);
    return () => sync.unwatch(selected);
  }, [sync, selected]);
  const session = sessions.find((x) => x.id === selected);

  return (
    <div className={s.window}>
      <aside className={s.sidebar} aria-label={t.sessions}>
        <div className={s.brand}>
          <Mark size={24} />
          <Wordmark />
        </div>
        <nav className={s.sessions} aria-label={t.sessions}>
          {sessions.length === 0 && <p className={s.quiet}>{t.noSessions}</p>}
          {sessions.map((x) => (
            <button key={x.id} className={s.session} aria-current={x.id === selected ? 'page' : undefined} onClick={() => setSelected(x.id)}>
              <span className={s.title}>{x.title || t.untitled}</span>
              {x.status !== 'idle' && <StatusBadge status={x.status} />}
            </button>
          ))}
        </nav>
        <ConnectionStatus />
      </aside>
      <main className={s.main}>
        <Banners />
        {session ? (
          <>
            <header className={s.header}>
              <h1>{session.title || t.untitled}</h1>
              <StatusBadge status={session.status} />
            </header>
            <Transcript id={session.id} />
          </>
        ) : (
          <p className={s.empty}>{t.chooseSession}</p>
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

function Transcript({ id }: { id: string }) {
  const t = useT();
  const transcript = useSyncState((st) => st.transcripts[id]);
  if (!transcript) return <p className={s.empty}>{t.loading}</p>;
  if (transcript.messages.length === 0) return <p className={s.empty}>{t.emptyTranscript}</p>;
  return (
    <div className={s.transcript} role="log" aria-label={t.conversation}>
      {transcript.messages.map((m) => (
        <MessageView key={m.id} message={m} />
      ))}
    </div>
  );
}

function MessageView({ message }: { message: Message }) {
  const t = useT();
  return (
    <article className={s.message} data-role={message.role}>
      <h2>{message.role === 'user' ? t.you : t.appName}</h2>
      <p>{message.text}</p>
    </article>
  );
}
