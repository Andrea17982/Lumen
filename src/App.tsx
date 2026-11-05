import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import {
  ArrowUp,
  Check,
  ChevronDown,
  ChevronLeft,
  Code2,
  Copy,
  Folder,
  Heart,
  Menu,
  MessageSquare,
  MoreHorizontal,
  Pin,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  Sparkles,
  Square,
  Sun,
  Moon,
  Monitor,
  Trash2,
  X,
  ArrowUpRight,
  AlertCircle,
  LoaderCircle,
  Pencil,
  Circle,
} from 'lucide-react';
import { applyEvent, call, desktop, external, subscribe } from './bridge';
import type {
  Account,
  Diagnostics,
  Message,
  Mode,
  Plan,
  Provider,
  RunEvent,
  Session,
  Status,
} from './types';

const REPO = 'https://github.com/Andrea17982/Lumen';
const MessageBody = lazy(() => import('./MessageBody'));
const PAYPAL =
  'https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen';
const active = (s?: Status) => s === 'running' || s === 'waiting';
const nameOf = (p: Provider) => (p === 'claude' ? 'Claude' : 'GPT');
function errorText(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}
function Mark({ small = false }: { small?: boolean }) {
  return (
    <span className={`lumen-mark ${small ? 'small' : ''}`} aria-hidden="true">
      <span />
      <span />
      <span />
    </span>
  );
}

export default function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [search, setSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [draft, setDraft] = useState('');
  const [mode, setMode] = useState<Mode>('auto');
  const [accountId, setAccountId] = useState('');
  const [model, setModel] = useState('');
  const [effort, setEffort] = useState('medium');
  const [writeAccess, setWriteAccess] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [settings, setSettings] = useState(false);
  const [sidebar, setSidebar] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [menu, setMenu] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState('');
  const [plan, setPlan] = useState<Plan | null>(null);
  const [theme, setTheme] = useState(() => localStorage.getItem('lumen-theme') || 'system');
  const [workspace, setWorkspace] = useState(() => localStorage.getItem('lumen-workspace') || '');
  const [copyState, setCopyState] = useState('');
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const bottom = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const settingsPanel = useRef<HTMLElement>(null);
  const reduced = useReducedMotion();
  // Filtering the sidebar must not discard the active conversation's workspace or run state.
  const activeSession = useRef<Session | undefined>(undefined);
  const session =
    sessions.find((s) => s.id === selected) ||
    (activeSession.current?.id === selected ? activeSession.current : undefined);
  if (session) activeSession.current = session;
  const running = active(session?.status) || busy;
  const currentWorkspace = session?.workspace || workspace;
  useEffect(() => {
    if (!settings) return;
    setSidebar(false);
    setMenu(false);
    const previous = document.activeElement as HTMLElement | null;
    const panel = settingsPanel.current;
    panel?.querySelector<HTMLElement>('button')?.focus();
    const trap = (e: KeyboardEvent) => {
      if (e.key !== 'Tab' || !panel) return;
      const controls = Array.from(
        panel.querySelectorAll<HTMLElement>(
          'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled)',
        ),
      ).filter((el) => el.getClientRects().length > 0);
      const first = controls[0],
        last = controls.at(-1);
      if (
        e.shiftKey &&
        (document.activeElement === first || !panel.contains(document.activeElement))
      ) {
        e.preventDefault();
        last?.focus();
      } else if (
        !e.shiftKey &&
        (document.activeElement === last || !panel.contains(document.activeElement))
      ) {
        e.preventDefault();
        first?.focus();
      }
    };
    document.addEventListener('keydown', trap);
    return () => {
      document.removeEventListener('keydown', trap);
      previous?.focus();
    };
  }, [settings]);

  const refresh = useCallback(async () => {
    const list = await call<Session[]>('list_sessions', { search, offset: 0 });
    setSessions(list);
    setOffset(0);
    setHasMore(list.length === 100);
  }, [search]);
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  const loadAccounts = useCallback(
    async () => setAccounts(await call<Account[]>('list_accounts')),
    [],
  );
  useEffect(() => {
    void refresh().catch((e) => setError(errorText(e)));
  }, [refresh]);
  useEffect(() => {
    void loadAccounts().catch((e) => setError(errorText(e)));
  }, [loadAccounts]);
  useEffect(() => {
    let gone = false;
    setMessages([]);
    setPlan(null);
    setMenu(false);
    setRenaming(false);
    if (selected)
      void call<Message[]>('get_messages', { id: selected })
        .then((m) => {
          if (!gone) setMessages(m);
        })
        .catch((e) => {
          if (!gone) setError(errorText(e));
        });
    return () => {
      gone = true;
    };
  }, [selected]);
  useEffect(() => {
    const media = matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === 'system' ? (media.matches ? 'dark' : 'light') : theme;
    };
    apply();
    media.addEventListener('change', apply);
    localStorage.setItem('lumen-theme', theme);
    return () => media.removeEventListener('change', apply);
  }, [theme]);
  useEffect(() => {
    let gone = false;
    let dispose = () => {};
    void subscribe((event: RunEvent) => {
      if (event.sessionId === selectedRef.current) setMessages((m) => applyEvent(m, event));
      if (event.kind === 'start' || event.kind === 'status' || event.kind === 'done') {
        const status = event.kind === 'start' ? 'waiting' : (event.text as Status);
        if (activeSession.current?.id === event.sessionId)
          activeSession.current = { ...activeSession.current, status };
        setSessions((list) => list.map((s) => (s.id === event.sessionId ? { ...s, status } : s)));
      }
      if (event.kind === 'done') {
        if (event.sessionId === selectedRef.current)
          void call<Message[]>('get_messages', { id: event.sessionId })
            .then((m) => {
              if (event.sessionId === selectedRef.current) setMessages(m);
            })
            .catch((e) => setError(errorText(e)));
        void refreshRef.current().catch((e) => setError(errorText(e)));
      }
    }).then((fn) => {
      if (gone) fn();
      else dispose = fn;
    });
    return () => {
      gone = true;
      dispose();
    };
  }, []);
  useEffect(() => {
    bottom.current?.scrollIntoView({ behavior: reduced ? 'instant' : 'smooth', block: 'end' });
  }, [messages, reduced]);
  useEffect(() => {
    const fn = (e: KeyboardEvent) => {
      if (settings) {
        if (e.key === 'Escape') setSettings(false);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'n') {
        e.preventDefault();
        setSelected(null);
        setMessages([]);
        input.current?.focus();
      }
      if (
        e.key === '/' &&
        !(
          e.target instanceof HTMLElement &&
          (['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName) || e.target.isContentEditable)
        )
      ) {
        e.preventDefault();
        setSidebar(true);
        requestAnimationFrame(() => searchInput.current?.focus());
      }
      if (e.key === 'Escape') {
        setMenu(false);
        setSidebar(false);
      }
    };
    window.addEventListener('keydown', fn);
    return () => window.removeEventListener('keydown', fn);
  }, [settings]);

  async function chooseFolder() {
    if (!desktop) {
      setWorkspace('Browser preview');
      return 'Browser preview';
    }
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({
      directory: true,
      multiple: false,
      title: 'Choose a trusted workspace',
    });
    if (typeof path === 'string') {
      setWorkspace(path);
      localStorage.setItem('lumen-workspace', path);
      return path;
    }
    return null;
  }
  async function send() {
    if (!draft.trim() || running) return;
    setError('');
    if (!desktop) {
      setError(
        'This is a browser preview. Run the desktop app to connect your accounts and start a conversation.',
      );
      return;
    }
    if (!accounts.length) {
      setSettings(true);
      return;
    }
    setBusy(true);
    try {
      let s = session;
      if (!s) {
        const folder = workspace || (await chooseFolder());
        if (!folder) return;
        s = await call<Session>('create_session', { workspace: folder });
        setSessions((list) => [s!, ...list]);
        setSelected(s.id);
      }
      const text = draft.trim();
      const result = await call<Plan>('send_message', {
        id: s.id,
        text,
        options: {
          mode,
          accountId: mode !== 'auto' && accountId ? accountId : null,
          model: model || null,
          effort,
          writeAccess,
        },
      });
      setDraft('');
      const snapshot = await call<Message[]>('get_messages', { id: s.id });
      if (selectedRef.current === s.id) {
        setPlan(result);
        setMessages(snapshot);
      }
      await refresh();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
      input.current?.focus();
    }
  }
  async function update(s: Session, title = s.title, pinned = s.pinned) {
    try {
      await call('update_session', { id: s.id, title, pinned });
      if (activeSession.current?.id === s.id) activeSession.current = { ...s, title, pinned };
      await refresh();
      setRenaming(false);
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function remove(s: Session) {
    try {
      await call('delete_session', { id: s.id });
      setSelected(null);
      await refresh();
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function loadMore() {
    try {
      const next = offset + 100;
      const rows = await call<Session[]>('list_sessions', { search, offset: next });
      setSessions((list) => [...list, ...rows]);
      setOffset(next);
      setHasMore(rows.length === 100);
    } catch (e) {
      setError(errorText(e));
    }
  }
  const newChat = () => {
    setSelected(null);
    setMessages([]);
    setDraft('');
    setSidebar(false);
    input.current?.focus();
  };

  return (
    <div className="app-shell">
      {sidebar && (
        <button
          className="sidebar-scrim"
          aria-label="Close sidebar"
          onClick={() => setSidebar(false)}
        />
      )}
      <aside inert={settings} className={`sidebar ${sidebar ? 'open' : ''}`}>
        <div className="brand">
          <Mark small />
          <span>Lumen</span>
          <span className="alpha">ALPHA</span>
        </div>
        <button className="new-chat" onClick={newChat}>
          <Plus size={17} />
          <span>New conversation</span>
          <kbd>Ctrl N</kbd>
        </button>
        <label className="search">
          <Search size={15} />
          <input
            ref={searchInput}
            aria-label="Search conversations"
            placeholder="Search conversations"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <kbd>/</kbd>
        </label>
        <div className="session-heading">
          <span>Your conversations</span>
          <span>
            {sessions.length}
            {hasMore ? '+' : ''}
          </span>
        </div>
        <nav className="session-list" aria-label="Conversations">
          {sessions.length === 0 && (
            <div className="sidebar-empty">
              <MessageSquare size={20} />
              <p>{search ? 'No conversations found.' : 'A little room for your next big idea.'}</p>
            </div>
          )}
          {sessions.map((s) => (
            <button
              key={s.id}
              className={`session-row ${selected === s.id ? 'selected' : ''}`}
              onClick={() => {
                setSelected(s.id);
                setSidebar(false);
              }}
            >
              <span className={`status-dot ${s.status}`} title={s.status} />
              <span className="session-title">{s.title}</span>
              {s.pinned && <Pin size={12} />}
              <span className="sr-only">{s.status}</span>
            </button>
          ))}
          {hasMore && (
            <button className="text-button load-more" onClick={() => void loadMore()}>
              Load more conversations
            </button>
          )}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-note">
            <ShieldCheck size={14} />
            <span>On your computer. In your control.</span>
          </div>
          <button className="settings-button" onClick={() => setSettings(true)}>
            <span className="avatar">A</span>
            <span>
              My workspace
              <small>
                {accounts.length} connected {accounts.length === 1 ? 'account' : 'accounts'}
              </small>
            </span>
            <Settings2 size={17} />
          </button>
        </div>
      </aside>

      <main inert={settings} className="workspace">
        <header className="topbar">
          <div className="topbar-left">
            <button
              className="icon-button mobile-menu"
              aria-label="Open sidebar"
              onClick={() => setSidebar(true)}
            >
              <Menu size={19} />
            </button>
            <span className="workspace-name">
              <Folder size={15} />
              {currentWorkspace
                ? currentWorkspace.split(/[\\/]/).filter(Boolean).at(-1)
                : 'Personal workspace'}
            </span>
            <span className="topbar-divider" />
            <span className="conversation-label">{session?.title || 'New conversation'}</span>
          </div>
          <div className="topbar-actions">
            {!desktop && <span className="preview-label">Browser preview</span>}
            <button className="icon-button" aria-label="Settings" onClick={() => setSettings(true)}>
              <Settings2 size={17} />
            </button>
            {session && (
              <div className="menu-anchor">
                <button
                  className="icon-button"
                  aria-label="Conversation actions"
                  aria-expanded={menu}
                  onClick={() => setMenu(!menu)}
                >
                  <MoreHorizontal size={20} />
                </button>
                {menu && (
                  <div className="popup action-menu">
                    <button
                      onClick={() => {
                        setRenameValue(session.title);
                        setRenaming(true);
                        setMenu(false);
                      }}
                    >
                      <Pencil size={14} />
                      Rename
                    </button>
                    <button
                      onClick={() => {
                        void update(session, session.title, !session.pinned);
                        setMenu(false);
                      }}
                    >
                      <Pin size={14} />
                      {session.pinned ? 'Unpin' : 'Pin conversation'}
                    </button>
                    <button
                      className="danger"
                      disabled={running}
                      onClick={() => {
                        void remove(session);
                        setMenu(false);
                      }}
                    >
                      <Trash2 size={14} />
                      Delete conversation
                    </button>
                  </div>
                )}
              </div>
            )}
          </div>
        </header>
        {renaming && session && (
          <form
            className="rename-bar"
            onSubmit={(e) => {
              e.preventDefault();
              void update(session, renameValue);
            }}
          >
            <input
              autoFocus
              aria-label="Conversation name"
              maxLength={100}
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
            />
            <button className="icon-button" type="submit" aria-label="Save name">
              <Check size={17} />
            </button>
            <button
              className="icon-button"
              type="button"
              aria-label="Cancel rename"
              onClick={() => setRenaming(false)}
            >
              <X size={17} />
            </button>
          </form>
        )}

        <div className={`conversation ${messages.length ? 'has-messages' : ''}`}>
          {messages.length === 0 ? (
            <div className="welcome">
              <motion.div
                initial={false}
                animate={{ y: [0, -5, 0] }}
                transition={
                  reduced ? { duration: 0 } : { duration: 8, repeat: Infinity, ease: 'easeInOut' }
                }
              >
                <Mark />
              </motion.div>
              <h1>
                A clear space
                <br />
                for a brighter idea.
              </h1>
              <p>Bring your thoughts. Let the right intelligence find you.</p>
              <div className="suggestions">
                <button
                  onClick={() => {
                    setDraft('Help me build a thoughtful plan for my next project.');
                    input.current?.focus();
                  }}
                >
                  <Code2 size={17} />
                  <span>Build something</span>
                  <ArrowUpRight size={14} />
                </button>
                <button
                  onClick={() => {
                    setDraft('Compare alternatives for my project. Ask me what you need to know.');
                    input.current?.focus();
                  }}
                >
                  <Sparkles size={17} />
                  <span>Explore an idea</span>
                  <ArrowUpRight size={14} />
                </button>
                <button
                  onClick={() => {
                    setDraft('Review the code in this workspace for bugs and security risks.');
                    input.current?.focus();
                  }}
                >
                  <ShieldCheck size={17} />
                  <span>A fresh perspective</span>
                  <ArrowUpRight size={14} />
                </button>
              </div>
            </div>
          ) : (
            <div className="message-list">
              {messages.map((m) => (
                <article key={m.id} className={`message ${m.role}`}>
                  <div className="message-author">
                    {m.role === 'assistant' ? (
                      <>
                        <Mark small />
                        <span>Lumen</span>
                      </>
                    ) : (
                      <>
                        <span className="user-mark">You</span>
                      </>
                    )}
                    {m.role === 'assistant' && m.text && (
                      <button
                        className="icon-button copy-button"
                        aria-label="Copy response"
                        onClick={() => {
                          void navigator.clipboard
                            .writeText(m.text)
                            .then(() => {
                              setCopyState(m.id);
                              setTimeout(() => setCopyState(''), 1800);
                            })
                            .catch((e) => setError(errorText(e)));
                        }}
                      >
                        {copyState === m.id ? <Check size={14} /> : <Copy size={14} />}
                      </button>
                    )}
                  </div>
                  {m.activity.length > 0 && (
                    <details className="activity">
                      <summary>
                        <span className="activity-indicator" />
                        {m.activity.some((a) => a.kind === 'error')
                          ? 'Activity & diagnostics'
                          : `${m.activity.length} activity ${m.activity.length === 1 ? 'event' : 'events'}`}
                        <ChevronDown size={13} />
                      </summary>
                      <div className="activity-content">
                        {m.activity.map((a, i) => (
                          <details key={i}>
                            <summary>
                              {a.kind === 'thinking'
                                ? 'Reasoning summary'
                                : a.kind === 'file'
                                  ? 'File changes'
                                  : a.kind.charAt(0).toUpperCase() + a.kind.slice(1)}
                            </summary>
                            <pre>{a.text}</pre>
                          </details>
                        ))}
                      </div>
                    </details>
                  )}
                  <div className="prose">
                    <Suspense fallback={<p>{m.text}</p>}>
                      <MessageBody text={m.text} />
                    </Suspense>
                  </div>
                  {!m.text && m.role === 'assistant' && running && (
                    <span className="working">
                      <LoaderCircle size={14} className="spin" />
                      Working on it…
                    </span>
                  )}
                </article>
              ))}
              <div ref={bottom} />
            </div>
          )}
        </div>

        <div className="composer-region">
          {error && (
            <div role="alert" className="error-banner">
              <AlertCircle size={16} />
              <span>{error}</span>
              <button
                className="icon-button"
                aria-label="Dismiss error"
                onClick={() => setError('')}
              >
                <X size={14} />
              </button>
            </div>
          )}
          {plan && (
            <div className="plan-line">
              <Sparkles size={13} />
              <span>
                {nameOf(plan.provider)} ·{' '}
                {plan.strategy === 'single'
                  ? 'Direct'
                  : plan.strategy === 'parallel'
                    ? 'Parallel perspectives'
                    : 'Writer + reviewer'}{' '}
                · {plan.effort} reasoning
              </span>
            </div>
          )}
          <div className="composer">
            <textarea
              ref={input}
              aria-label="Message"
              placeholder="What's on your mind?"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  void send();
                }
              }}
              maxLength={100000}
            />
            {advanced && mode !== 'auto' && (
              <div className="advanced-controls">
                <label>
                  Account
                  <select
                    aria-label="Account"
                    value={accountId}
                    onChange={(e) => setAccountId(e.target.value)}
                  >
                    <option value="">First {nameOf(mode)} account</option>
                    {accounts
                      .filter((a) => a.provider === mode)
                      .map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.name}
                        </option>
                      ))}
                  </select>
                </label>
                <label>
                  Model
                  <input
                    aria-label="Model"
                    placeholder="Provider default"
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                    maxLength={100}
                  />
                </label>
                <label>
                  Reasoning
                  <select
                    aria-label="Reasoning effort"
                    value={effort}
                    onChange={(e) => setEffort(e.target.value)}
                  >
                    <option value="low">Low</option>
                    <option value="medium">Medium</option>
                    <option value="high">High</option>
                  </select>
                </label>
              </div>
            )}
            <div className="composer-toolbar">
              <div className="mode-control" role="group" aria-label="AI provider">
                {(['auto', 'claude', 'gpt'] as const).map((p) => (
                  <button
                    key={p}
                    aria-pressed={mode === p}
                    className={mode === p ? 'active' : ''}
                    onClick={() => {
                      setMode(p);
                      setAccountId('');
                      setModel('');
                    }}
                  >
                    {p === 'auto' && <Sparkles size={13} />}
                    <span>{p === 'auto' ? 'Auto' : nameOf(p)}</span>
                  </button>
                ))}
              </div>
              {mode !== 'auto' && (
                <button
                  className={`icon-button advanced-toggle ${advanced ? 'on' : ''}`}
                  aria-label="Advanced provider options"
                  aria-expanded={advanced}
                  onClick={() => setAdvanced(!advanced)}
                >
                  <ChevronDown size={15} />
                </button>
              )}
              <div className="composer-spacer" />
              <button
                className={`access-control ${writeAccess ? 'enabled' : ''}`}
                aria-pressed={writeAccess}
                onClick={() => setWriteAccess(!writeAccess)}
                title="Enable file edits only in a trusted workspace"
              >
                <ShieldCheck size={13} />
                <span>{writeAccess ? 'File edits on' : 'Read only'}</span>
              </button>
              <button
                className="send-button"
                aria-label={running ? 'Stop response' : 'Send message'}
                disabled={!running && !draft.trim()}
                onClick={() => {
                  if (running && selected)
                    void call('cancel_run', { id: selected }).catch((e) => setError(errorText(e)));
                  else void send();
                }}
              >
                {running ? <Square size={15} fill="currentColor" /> : <ArrowUp size={19} />}
              </button>
            </div>
          </div>
          <div className="composer-caption">
            <span>
              {mode === 'auto'
                ? 'The right model, thoughtfully chosen.'
                : `${nameOf(mode)} · Your account. Your conversation.`}
            </span>
            <span>
              Enter to send <span className="caption-dot">·</span> Shift Enter for a new line
            </span>
          </div>
        </div>
        <footer className="workspace-footer">
          <span>
            <span className="footer-dot" />A quieter kind of intelligence.
          </span>
          <button onClick={() => void external(REPO).catch((e) => setError(errorText(e)))}>
            Completely AI Written <ArrowUpRight size={11} />
          </button>
        </footer>
      </main>

      <AnimatePresence>
        {settings && (
          <motion.aside
            ref={settingsPanel}
            role="dialog"
            aria-modal="true"
            className="settings-panel"
            initial={reduced ? false : { x: 35, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={reduced ? { x: 0, opacity: 1 } : { x: 35, opacity: 0 }}
            transition={{ duration: reduced ? 0 : 0.2 }}
            aria-label="Settings panel"
          >
            <div className="settings-header">
              <button className="text-button" onClick={() => setSettings(false)}>
                <ChevronLeft size={16} />
                Workspace
              </button>
              <button
                className="icon-button"
                aria-label="Close settings"
                onClick={() => setSettings(false)}
              >
                <X size={18} />
              </button>
            </div>
            <h2>
              Make yourself
              <br />
              at home.
            </h2>
            <p className="settings-intro">Your accounts. A little less friction.</p>
            <AccountSettings accounts={accounts} refresh={loadAccounts} onError={setError} />
            <section className="settings-section">
              <h3>Appearance</h3>
              <div className="theme-switch" role="group" aria-label="Theme">
                {[
                  { id: 'light', Icon: Sun },
                  { id: 'dark', Icon: Moon },
                  { id: 'system', Icon: Monitor },
                ].map(({ id, Icon }) => (
                  <button
                    key={id}
                    aria-pressed={theme === id}
                    className={theme === id ? 'active' : ''}
                    onClick={() => setTheme(id)}
                  >
                    <Icon size={16} />
                    {id.charAt(0).toUpperCase() + id.slice(1)}
                  </button>
                ))}
              </div>
            </section>
            <section className="settings-section">
              <h3>Workspace folder</h3>
              <p>New conversations start here. Choose a folder you trust.</p>
              <button
                className="folder-button"
                onClick={() => void chooseFolder().catch((e) => setError(errorText(e)))}
              >
                <Folder size={16} />
                <span>{workspace || 'Choose a folder'}</span>
                <ArrowUpRight size={14} />
              </button>
            </section>
            <section className="settings-section about">
              <Mark small />
              <h3>
                Built with AI.
                <br />
                Shared with everyone.
              </h3>
              <p>
                Lumen is completely AI Written, created at the direction of Andrea. Thank you to
                everyone who tries it and shares how it feels.
              </p>
              <button
                className="text-button"
                onClick={() => void external(`${REPO}/issues`).catch((e) => setError(errorText(e)))}
              >
                Share your feedback <ArrowUpRight size={14} />
              </button>
              <button
                className="donate-button"
                onClick={() => void external(PAYPAL).catch((e) => setError(errorText(e)))}
              >
                <Heart size={15} />
                Support Lumen on PayPal
                <ArrowUpRight size={14} />
              </button>
              <small>Optional donations · No features behind a paywall.</small>
            </section>
            <div className="version">Lumen 0.1.0 · Open source · MIT</div>
          </motion.aside>
        )}
      </AnimatePresence>
    </div>
  );
}

function AccountSettings({
  accounts,
  refresh,
  onError,
}: {
  accounts: Account[];
  refresh: () => Promise<void>;
  onError: (text: string) => void;
}) {
  const [name, setName] = useState('');
  const [provider, setProvider] = useState<Provider>('claude');
  const [statuses, setStatuses] = useState<Record<string, string>>({});
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [adding, setAdding] = useState(false);
  const [pending, setPending] = useState(false);
  const [confirm, setConfirm] = useState('');
  useEffect(() => {
    void call<Diagnostics>('diagnostics')
      .then(setDiag)
      .catch((e) => onError(errorText(e)));
  }, [onError]);
  async function action(fn: () => Promise<unknown>) {
    setPending(true);
    try {
      await fn();
    } catch (e) {
      onError(errorText(e));
    } finally {
      setPending(false);
    }
  }
  return (
    <section className="settings-section">
      <div className="section-title">
        <h3>Connected accounts</h3>
        <button className="icon-button" aria-label="Add account" onClick={() => setAdding(!adding)}>
          <Plus size={17} />
        </button>
      </div>
      <p>Sign in through the official CLI. Each account gets its own configuration folder.</p>
      {diag && (
        <div className="cli-status">
          <span className={diag.claude ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Claude CLI {diag.claude ? 'found' : 'not found'}
          </span>
          <span className={diag.gpt ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Codex CLI {diag.gpt ? 'found' : 'not found'}
          </span>
        </div>
      )}
      {accounts.length === 0 && (
        <div className="account-empty">Connect an account to start your first conversation.</div>
      )}
      {accounts.map((a) => (
        <div className="account-row" key={a.id}>
          <div className={`provider-avatar ${a.provider}`}>
            {a.provider === 'claude' ? 'C' : 'G'}
          </div>
          <div className="account-info">
            <strong>{a.name}</strong>
            <small>
              {nameOf(a.provider)} · {statuses[a.id] || 'Connection unchecked'}
            </small>
          </div>
          <button
            className="text-button"
            disabled={pending}
            onClick={() =>
              void action(async () => {
                await call('login_account', { id: a.id });
                setStatuses((s) => ({ ...s, [a.id]: 'Finish login in terminal' }));
              })
            }
          >
            Sign in
          </button>
          <button
            className="icon-button"
            disabled={pending}
            aria-label={`Check ${a.name}`}
            onClick={() =>
              void action(async () => {
                const ready = await call<boolean>('account_status', { id: a.id });
                setStatuses((s) => ({ ...s, [a.id]: ready ? 'Connected' : 'Sign-in required' }));
              })
            }
          >
            <ShieldCheck size={15} />
          </button>
          <button
            className="icon-button"
            disabled={pending}
            aria-label={`Remove ${a.name}`}
            onClick={() => setConfirm(a.id)}
          >
            <Trash2 size={14} />
          </button>
          {confirm === a.id && (
            <div className="account-confirm">
              <span>Remove this account and its local login files?</span>
              <button
                className="text-button danger"
                disabled={pending}
                onClick={() =>
                  void action(async () => {
                    await call('remove_account', { id: a.id });
                    await refresh();
                    setConfirm('');
                  })
                }
              >
                Remove
              </button>
              <button className="text-button" onClick={() => setConfirm('')}>
                Keep
              </button>
            </div>
          )}
        </div>
      ))}
      {adding && (
        <form
          className="account-form"
          onSubmit={(e) => {
            e.preventDefault();
            void action(async () => {
              await call('add_account', { name, provider });
              setName('');
              setAdding(false);
              await refresh();
            });
          }}
        >
          <label>
            Account name
            <input
              required
              maxLength={80}
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Personal"
            />
          </label>
          <label>
            Provider
            <select value={provider} onChange={(e) => setProvider(e.target.value as Provider)}>
              <option value="claude">Claude Code</option>
              <option value="gpt">ChatGPT / Codex</option>
            </select>
          </label>
          <button className="primary-button" disabled={pending || !name.trim()} type="submit">
            {pending ? 'Adding…' : 'Add account'}
          </button>
        </form>
      )}
      <a
        className="docs-link"
        href={`${REPO}/blob/main/docs/accounts.md`}
        target="_blank"
        rel="noreferrer"
        onClick={(e) => {
          e.preventDefault();
          void external(`${REPO}/blob/main/docs/accounts.md`).catch((err) =>
            onError(errorText(err)),
          );
        }}
      >
        Account setup & provider requirements
        <ArrowUpRight size={12} />
      </a>
    </section>
  );
}
