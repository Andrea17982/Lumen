import { useEffect, useRef, useState } from 'react';
import { Badge, Button, IconButton, Menu, type Tone } from '../../design/components';
import { useAppearance } from '../../design/appearance';
import { I18nProvider, systemLocale, useT, type Locale } from '../../i18n';
import { Conversation } from '../../features/conversation/Conversation';
import { ProjectPanel } from '../../features/sessions/ProjectPanel';
import { LogoButton } from '../../brand/Logo';
import { currentProject, inProject, sessionTitle } from '../../features/sessions/sessions';
import { Sidebar } from '../../features/sessions/Sidebar';
import type { Language, Preferences } from '../../features/settings/General';
import { Settings } from '../../features/settings/Settings';
import { Welcome } from '../../features/welcome/Welcome';
import { Tour } from '../../features/welcome/Tour';
import welcomeStyles from '../../features/welcome/Welcome.module.css';
import { WorktreeBar } from '../../features/worktree/Worktree';
import { usePluginThemeStyle } from '../../features/settings/useThemeStyle';
import { useNotifications } from './notifications';
import { isSessionActive, type Connection, type Session, type UpdateStatus } from '../../protocol/types';
import { errorText } from '../../transport/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import s from './App.module.css';

const tones: Record<string, Tone> = { running: 'accent', queued: 'neutral', waiting: 'warning', interrupted: 'warning', partial: 'warning', completed: 'success', failed: 'danger', error: 'danger' };

/**
 * Desktop surface: the daemon's sessions and the selected conversation, kept in sync across
 * reconnections.
 */
export function App() {
  const [appearance, setAppearance] = useAppearance();
  const [language, chooseLanguage] = useState<Language>(() => {
    const saved = remembered('lumen.language');
    return saved === 'en' || saved === 'it' ? saved : 'system';
  });
  // Tests pin the language in the URL; otherwise the saved choice or the system's.
  const query = new URLSearchParams(location.search).get('locale');
  const locale: Locale = query === 'it' || query === 'en' ? query : language === 'system' ? systemLocale() : language;
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);
  const setLanguage = (next: Language) => {
    chooseLanguage(next);
    remember('lumen.language', next === 'system' ? null : next);
  };
  return (
    <I18nProvider locale={locale}>
      <Shell preferences={{ appearance, setAppearance, language, setLanguage }} />
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

function Shell({ preferences }: { preferences: Preferences }) {
  const t = useT();
  usePluginThemeStyle();
  useNotifications();
  const sync = useSyncClient();
  const sessions = useSyncState((st) => st.sessions);
  const workspaces = useSyncState((st) => st.workspaces);
  const [selected, setSelected] = useState<string | null>(() => remembered('lumen.session'));
  const [outsideList, setOutsideList] = useState<Session | null>(null);
  const [projectId, setProjectId] = useState<string | null>(() => remembered('lumen.project'));
  const [hidden, setHidden] = useState(() => remembered('lumen.sidebar') === 'hidden');
  const [details, setDetails] = useState(false);
  const [settings, setSettings] = useState(false);
  const [welcome, setWelcome] = useState<boolean | null>(null);
  const [tour, setTour] = useState<number | null>(null);
  const [inviteError, setInviteError] = useState(false);
  const tourRoot = useRef<HTMLDivElement>(null);
  const main = useRef<HTMLElement>(null);
  const onboarding = useSyncState((st) => st.onboarding);
  const project = currentProject(workspaces, sessions, projectId);
  const synced = useSyncState((st) => st.synced);
  const showWelcome = welcome ?? (onboarding?.setup === 'pending' && !onboarding.existing);
  const tourSection = tour === 6 ? 'general' : tour === 7 ? 'maintenance' : undefined;
  useEffect(() => {
    if (!selected || !synced || sessions.some((s) => s.id === selected)) return;
    let live = true;
    sync.call<Session>('get_session', { id: selected }).then(
      (session) => { if (live) setOutsideList(session); },
      () => { if (live) setOutsideList(null); },
    );
    return () => { live = false; };
  }, [sync, selected, synced, sessions]);
  const candidate = sessions.find((x) => x.id === selected) ?? (outsideList?.id === selected ? outsideList : null);
  const session = candidate && project && inProject(candidate, project) ? candidate : null;
  const activeId = session?.id;
  useEffect(() => {
    if (!activeId) return;
    sync.watch(activeId);
    return () => sync.unwatch(activeId);
  }, [sync, activeId]);
  // Once chosen, the open project stays put: moving or updating a conversation never switches it.
  useEffect(() => {
    if (project && project.id !== projectId) setProjectId(project.id);
  }, [project, projectId]);
  const connected = useSyncState((st) => st.connection.state === 'connected');
  const [error, setError] = useState<string | null>(null);
  const busy = !!session && isSessionActive(session.status);

  const hide = (value: boolean) => {
    setHidden(value);
    remember('lumen.sidebar', value ? 'hidden' : null);
  };
  useEffect(() => {
    if (showWelcome) return;
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
    remember('lumen.session', id);
    if (id && project) remember('lumen.project', project.id);
  }

  function openProject(id: string | null) {
    setProjectId(id);
    remember('lumen.project', id);
    if (id !== project?.id) {
      setSelected(null);
      remember('lumen.session', null);
    }
  }

  // Handoff: the daemon writes a new conversation seeded with this one's goal and changes.
  async function compact(id: string) {
    setError(null);
    try {
      const next = await sync.mutate<Session>('compact_session', { id });
      choose(next.id);
    } catch (e) {
      setError(String(e));
    }
  }

  function closeWelcome(startTour: boolean) {
    setWelcome(false);
    setSettings(false);
    if (startTour) setTour(0);
    else requestAnimationFrame(() => main.current?.focus());
  }

  function closeTour() {
    setTour(null);
    requestAnimationFrame(() => main.current?.focus());
  }

  if (showWelcome) return <Welcome preferences={preferences} project={project} onProject={openProject} onClose={closeWelcome} />;

  return (
    <div className={s.window} data-sidebar={hidden ? 'hidden' : undefined} ref={tourRoot}>
      {!hidden && <Sidebar project={project} selected={selected} onSelect={choose} onProject={openProject} onHide={() => hide(true)} onSettings={() => setSettings(!settings)} footer={<ConnectionStatus />} />}
      <main className={s.main} ref={main} tabIndex={-1}>
        <Banners />
        {tour !== null && <Tour index={tour} onStep={setTour} root={tourRoot} onClose={closeTour} />}
        {tour === null && !settings && onboarding?.existing && onboarding.setup === 'pending' && welcome !== false && (
          <aside className={welcomeStyles.invite} aria-label={t.welcome.invite}>
            <span>{t.welcome.invite}</span>
            <Button variant="plain" onPress={() => setWelcome(true)}>{t.welcome.reopen}</Button>
            <IconButton icon="close" label={t.welcome.dismiss} onPress={() => {
              setInviteError(false);
              void sync.updateOnboarding('skipSetup').catch(() => setInviteError(true));
            }} />
            {inviteError && <span role="alert">{t.welcome.saveError}</span>}
          </aside>
        )}
        {settings || tourSection ? (
          <Settings key={tourSection ?? 'settings'} initialSection={tourSection} preferences={preferences} onClose={() => { setSettings(false); if (tourSection) closeTour(); }} onWelcome={() => { setTour(null); setWelcome(true); }} onTour={() => { setSettings(false); setTour(0); }} leading={hidden && <LogoButton label={t.sidebar.show} onPress={() => hide(false)} />} />
        ) : session ? (
          <>
            <header className={s.header}>
              {hidden && <LogoButton label={t.sidebar.show} onPress={() => hide(false)} />}
              <h1>{sessionTitle(session, t.untitled)}</h1>
              <StatusBadge status={session.status} />
              <span className={s.spacer} />
              <IconButton icon="folder" label={t.project.open} onPress={() => setDetails(!details)} />
              <Menu label={t.moreActions} items={[{ id: 'compact', label: t.recovery.compact, isDisabled: !connected || busy }]} onAction={() => void compact(session.id)} />
            </header>
            <WorktreeBar key={session.id} session={session} />
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
                <LogoButton label={t.sidebar.show} onPress={() => hide(false)} />
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
  const error = useSyncState((st) => st.error);
  const update = useSyncState((st) => st.update);
  const [later, setLater] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  // A Go back that could not finish at startup: the shell kept the current data and says why.
  useEffect(() => {
    sync.call<UpdateStatus>('update_status').then((st) => setProblem(st.problem), () => {});
  }, [sync]);
  return (
    <div className={s.banners}>
      <ConnectionBanner connection={connection} stale={!synced && sessions.length > 0} />
      {problem && (
        <div className={s.banner} data-tone="warning" role="alert">
          <span>{t.banner.goBackFailed(problem)}</span>
          <Button variant="secondary" onPress={() => setProblem(null)}>
            {t.banner.checked}
          </Button>
        </div>
      )}
      {update && later !== update.version && (
        <div className={s.banner} role="status">
          <span>{updateError ?? t.banner.update(update.version)}</span>
          <Button
            variant="primary"
            isDisabled={installing}
            onPress={() => {
              setInstalling(true);
              setUpdateError(null);
              // On success the app restarts; only a failure comes back here.
              sync
                .call(update.installable ? 'update_install' : 'update_open_page')
                .catch((e) => setUpdateError(errorText(e)))
                .finally(() => setInstalling(false));
            }}
          >
            {t.banner.updateNow}
          </Button>
          <Button variant="secondary" onPress={() => setLater(update.version)}>
            {t.banner.later}
          </Button>
        </div>
      )}
      {error && connection.state === 'connected' && (
        <div className={s.banner} data-tone="warning" role="alert">
          <span>{t.banner.syncError(error)}</span>
          <Button variant="secondary" onPress={() => void sync.resync()}>
            {t.banner.retry}
          </Button>
        </div>
      )}
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
  const sync = useSyncClient();
  const [repairing, setRepairing] = useState(false);
  const [repairError, setRepairError] = useState<string | null>(null);
  // The shell restarts the service itself: this works while the service cannot answer.
  const repair = (
    <>
      <Button
        variant="secondary"
        isDisabled={repairing}
        onPress={() => {
          setRepairing(true);
          setRepairError(null);
          sync
            .call('restart_service')
            .catch((e) => setRepairError(errorText(e)))
            .finally(() => setRepairing(false));
        }}
      >
        {t.banner.repair}
      </Button>
      {repairError && <span role="alert">{repairError}</span>}
    </>
  );
  if (connection.state === 'mismatch')
    return (
      <div className={s.banner} data-tone="warning" role="alert">
        <span>{t.banner.mismatch}</span>
        {repair}
      </div>
    );
  if (connection.state === 'failed')
    return (
      <div className={s.banner} data-tone="danger" role="alert" title={connection.detail}>
        <span>
          {t.banner.failed} {connection.detail}
        </span>
        {repair}
      </div>
    );
  if (connection.state === 'reconnecting' || (connection.state === 'connected' && stale)) return <p className={s.banner}>{t.banner.stale}</p>;
  return null;
}
