import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import { motion, useReducedMotion } from 'framer-motion';
import {
  ArrowUp,
  Check,
  ChevronDown,
  ChevronUp,
  Copy,
  Folder,
  Menu,
  MessageSquare,
  Pin,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  Sparkles,
  Square,
  X,
  AlertCircle,
  LoaderCircle,
  Circle,
  RotateCcw,
  Paperclip,
  BookOpen,
  Repeat2,
  Columns2,
  Shrink,
  Workflow,
} from 'lucide-react';
import ProviderOptions from './components/ProviderOptions';
import ProjectPanel from './components/ProjectPanel';
import QuitConfirm from './components/QuitConfirm';
import OrchestraCard from './components/OrchestraCard';
import { saveSettings, useAppSettings } from './lib/settings';
import SettingsPanel, { type SettingsTab } from './settings/SettingsPanel';
import { applyEvent, call, desktop, subscribe } from './lib/bridge';
import type {
  Account,
  AccessMode,
  Workspace,
  Message,
  Mode,
  Plan,
  AccountUsage,
  UsageWindow,
  FileEntry,
  Health,
  RewindPreview,
  RunEvent,
  Session,
  Status,
  OrchestraState,
} from './lib/types';
import { errorText, Mark, nameOf } from './lib/util';

const MessageBody = lazy(() => import('./components/MessageBody'));
const active = (s?: Status) => s === 'running' || s === 'waiting';
const HIDDEN = [
  'checkpoint',
  'lane',
  'worktree',
  'applied',
  'discarded',
  'attachments',
  'orchestra',
  'orchestra-nogit',
];
function orchestraOf(m: Message): OrchestraState | null {
  const a = m.activity.find((x) => x.kind === 'orchestra');
  if (!a) return null;
  try {
    return JSON.parse(a.text) as OrchestraState;
  } catch {
    return null;
  }
}
const laneOf = (m: Message) => m.activity.find((a) => a.kind === 'lane')?.text;
/** Which provider wrote a message, from its lane or the writer's model activity. */
function providerOf(m: Message): 'claude' | 'gpt' | null {
  const lane = laneOf(m);
  const models = m.activity.filter((a) => a.kind === 'model').map((a) => a.text);
  const fell = m.activity.some((a) => a.kind === 'fallback');
  const src = lane || (fell ? models.at(-1) : models[0]) || '';
  if (src.startsWith('claude')) return 'claude';
  if (src.startsWith('codex')) return 'gpt';
  return null;
}
const tokens = (text: string) => Math.ceil(text.length / 4);
const compactTokens = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n));
function countdown(until: number) {
  const m = Math.max(0, Math.ceil((until - Date.now()) / 60000));
  return m >= 60 ? `${Math.floor(m / 60)}h ${m % 60}m` : `${m}m`;
}
function usageBar(label: string, window?: UsageWindow) {
  return (
    <div className="usage-meter">
      <span>{label}</span>
      {window ? (
        <>
          <div className="usage-track">
            <i style={{ width: `${window.usedPercent}%` }} />
          </div>
          <strong>{Math.round(window.usedPercent)}%</strong>
        </>
      ) : (
        <small>Unavailable</small>
      )}
    </div>
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
  const [autoPolicy, setAutoPolicy] = useState(
    () => localStorage.getItem('lumen-auto-policy') || 'default',
  );
  const [balanceUsage, setBalanceUsage] = useState(
    () => localStorage.getItem('lumen-balance-usage') === 'true',
  );
  const [projectForm, setProjectForm] = useState(false);
  const [projectName, setProjectName] = useState('');
  const [projectFilter, setProjectFilter] = useState('*');
  const [context, setContext] = useState<{ session: Session; x: number; y: number } | null>(null);
  const [renameTarget, setRenameTarget] = useState<Session | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Session | null>(null);
  const [mode, setMode] = useState<Mode>('auto');
  const [accountId, setAccountId] = useState('');
  const [model, setModel] = useState('');
  const [effort, setEffort] = useState('');
  const [accessMode, setAccessMode] = useState<AccessMode>('read-only');
  const [draggedSession, setDraggedSession] = useState<string | null>(null);
  const [catalogReady, setCatalogReady] = useState(false);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [workspaceId, setWorkspaceId] = useState(
    () => localStorage.getItem('lumen-workspace-id') || '',
  );
  const [folderBusy, setFolderBusy] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [settings, setSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>('general');
  const openSettings = (tab: SettingsTab = 'general') => {
    setSettingsTab(tab);
    setSettings(true);
  };
  const [sidebar, setSidebar] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState('');
  const [plan, setPlan] = useState<Plan | null>(null);
  const [usage, setUsage] = useState<AccountUsage[]>([]);
  const [usageOpen, setUsageOpen] = useState(false);
  const [theme, setTheme] = useState(() => localStorage.getItem('lumen-theme') || 'system');
  const [workspace, setWorkspace] = useState(() => localStorage.getItem('lumen-workspace') || '');
  const [copyState, setCopyState] = useState('');
  const [autoReview, setAutoReview] = useState(
    () => localStorage.getItem('lumen-auto-review') === 'true',
  );
  const [attached, setAttached] = useState<string[]>([]);
  const appSettings = useAppSettings();
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [projectOpen, setProjectOpen] = useState(false);
  const [health, setHealth] = useState<Health[]>([]);
  const [rewindTarget, setRewindTarget] = useState<{ id: string; preview: RewindPreview } | null>(
    null,
  );
  const [, setTick] = useState(0);
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const bottom = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const reduced = useReducedMotion();
  // Filtering the sidebar must not discard the active conversation's workspace or run state.
  const activeSession = useRef<Session | undefined>(undefined);
  const session =
    sessions.find((s) => s.id === selected) ||
    (activeSession.current?.id === selected ? activeSession.current : undefined);
  if (session) activeSession.current = session;
  const running = active(session?.status) || busy;
  const currentWorkspace = session?.workspace || workspace;
  const currentWorkspaceId = session ? session.workspaceId || '' : workspaceId;
  const namedWorkspace = workspaces.find((w) => w.id === currentWorkspaceId);
  useEffect(() => {
    void call<Workspace[]>('list_workspaces')
      .then(setWorkspaces)
      .catch((e) => setError(errorText(e)));
  }, []);
  // Settings behaves like a modal for the keyboard: focus moves in, stays in, and returns.
  const settingsOpener = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (!settings) {
      settingsOpener.current?.focus();
      return;
    }
    settingsOpener.current = document.activeElement as HTMLElement | null;
    requestAnimationFrame(() =>
      document.querySelector<HTMLElement>('.settings-panel button:not(:disabled)')?.focus(),
    );
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
  const loadUsage = useCallback(async () => {
    setUsage(await call<AccountUsage[]>('get_usage'));
    for (const a of accounts.filter((account) => account.provider === 'gpt')) {
      try {
        const row = await call<AccountUsage>('refresh_usage', { id: a.id });
        setUsage((old) => [...old.filter((x) => x.accountId !== row.accountId), row]);
      } catch {
        /* Keep any saved snapshot when the CLI cannot refresh. */
      }
    }
  }, [accounts]);
  useEffect(() => {
    let stopped = false;
    const refresh = async () => {
      try {
        const rows = await call<AccountUsage[]>('get_usage');
        if (!stopped) setUsage(rows);
        for (const a of accounts.filter((account) => account.provider === 'gpt')) {
          try {
            const row = await call<AccountUsage>('refresh_usage', { id: a.id });
            if (!stopped)
              setUsage((old) => [...old.filter((x) => x.accountId !== row.accountId), row]);
          } catch {
            /* Keep the most recent saved snapshot when the CLI is unavailable. */
          }
        }
      } catch {
        /* Usage is optional when running in browser preview. */
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 30_000);
    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [accounts]);
  useEffect(() => {
    void refresh().catch((e) => setError(errorText(e)));
  }, [refresh]);
  useEffect(() => {
    const load = () =>
      void call<Health[]>('provider_health')
        .then(setHealth)
        .catch(() => {});
    load();
    const t = window.setInterval(() => {
      load();
      setTick((n) => n + 1);
    }, 15_000);
    return () => window.clearInterval(t);
  }, [accounts, sessions]);
  useEffect(() => {
    void loadAccounts().catch((e) => setError(errorText(e)));
  }, [loadAccounts]);
  useEffect(() => {
    let gone = false;
    setMessages([]);
    setPlan(null);
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
    if (session?.accessMode) setAccessMode(session.accessMode);
  }, [selected, session?.accessMode]);
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
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'p') {
        e.preventDefault();
        setSidebar(true);
        setProjectForm(true);
        return;
      }
      if (settings) {
        if (e.key === 'Escape') setSettings(false);
        if (e.key === 'Tab') {
          const items = [
            ...document.querySelectorAll<HTMLElement>(
              '.settings-panel button:not(:disabled), .settings-panel input:not(:disabled), .settings-panel select:not(:disabled), .settings-panel textarea, .settings-panel summary',
            ),
          ].filter((el) => el.offsetParent !== null);
          const first = items[0];
          const last = items.at(-1);
          if (e.shiftKey && document.activeElement === first) {
            e.preventDefault();
            last?.focus();
          } else if (!e.shiftKey && document.activeElement === last) {
            e.preventDefault();
            first?.focus();
          }
        }
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
        setContext(null);
        setProjectForm(false);
        setSidebar(false);
      }
    };
    window.addEventListener('keydown', fn);
    return () => window.removeEventListener('keydown', fn);
  }, [settings]);

  async function pickFolder() {
    if (!desktop) return 'Browser preview';
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({
      directory: true,
      multiple: false,
      title: 'Choose conversation folder',
      defaultPath: currentWorkspace || undefined,
    });
    return typeof path === 'string' ? path : null;
  }
  async function bindFolder(folder: string, wid: string) {
    if (session) {
      const updated = await call<Session>('set_session_workspace', {
        id: session.id,
        workspaceId: wid || null,
        folder,
      });
      activeSession.current = updated;
      setSessions((list) => list.map((s) => (s.id === updated.id ? updated : s)));
    }
    setWorkspace(folder);
    setWorkspaceId(wid);
    localStorage.setItem('lumen-workspace', folder);
    localStorage.setItem('lumen-workspace-id', wid);
  }
  async function chooseFolder() {
    setFolderBusy(true);
    try {
      const folder = await pickFolder();
      if (folder) await bindFolder(folder, currentWorkspaceId);
      return folder;
    } finally {
      setFolderBusy(false);
    }
  }
  async function chooseWorkspace(id: string) {
    setFolderBusy(true);
    try {
      if (id === '__add') {
        setProjectForm(true);
        setSidebar(true);
        return;
      }
      if (session) await moveSession(session, id);
      else {
        setWorkspaceId(id);
        localStorage.setItem('lumen-workspace-id', id);
      }
    } catch (e) {
      setError(errorText(e));
    } finally {
      setFolderBusy(false);
    }
  }
  async function createProject() {
    try {
      const project = await call<Workspace>('add_workspace', {
        name: projectName.trim(),
        folder: '',
      });
      setWorkspaces(await call<Workspace[]>('list_workspaces'));
      setWorkspaceId(project.id);
      localStorage.setItem('lumen-workspace-id', project.id);
      setProjectFilter(project.id);
      setSelected(null);
      setProjectName('');
      setProjectForm(false);
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function moveSession(s: Session, id: string) {
    try {
      const updated = await call<Session>('set_session_workspace', {
        id: s.id,
        workspaceId: id || null,
        folder: s.workspace,
      });
      if (activeSession.current?.id === s.id) activeSession.current = updated;
      await refresh();
      setContext(null);
    } catch (e) {
      setError(errorText(e));
    }
  }
  /** Options shared by a new turn and by Orchestra's follow-up steps. */
  function runOptions(override?: { mode: Mode; auditOf: string }) {
    const manual = !override && (mode === 'claude' || mode === 'gpt');
    return {
      mode: override?.mode ?? mode,
      accountId: manual && accountId ? accountId : null,
      model: manual ? model || null : null,
      effort: manual ? effort : '',
      accessMode: override ? 'read-only' : accessMode,
      autoPolicy,
      balanceUsage,
      autoReview: override ? false : autoReview,
      attachments: override ? [] : attached,
      auditOf: override?.auditOf ?? null,
    };
  }
  async function orchestra(command: string, args: Record<string, unknown>) {
    if (!session) return;
    setBusy(true);
    setError('');
    try {
      await call(command, { id: session.id, options: { ...runOptions(), mode: 'auto' }, ...args });
      setMessages(await call<Message[]>('get_messages', { id: session.id }));
      await refresh();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  async function send(override?: { text: string; mode: Mode; auditOf: string }) {
    const text = override?.text ?? draft.trim();
    const runMode = override?.mode ?? mode;
    if (!text || running || folderBusy) return;
    if (!override && (mode === 'claude' || mode === 'gpt') && !catalogReady) {
      setError('Load the official model catalog before sending.');
      return;
    }
    setError('');
    if (!desktop) {
      setError(
        'This is a browser preview. Run the desktop app to connect your accounts and start a conversation.',
      );
      return;
    }
    if (!accounts.length) {
      openSettings('accounts');
      return;
    }
    setBusy(true);
    try {
      let s = session;
      if (!s) {
        const folder = workspace || (await chooseFolder());
        if (!folder) return;
        s = await call<Session>('create_session', { workspace: folder });
        if (workspaceId)
          s = await call<Session>('set_session_workspace', { id: s.id, workspaceId, folder });
        setSessions((list) => [s!, ...list]);
        setSelected(s.id);
      }
      const result = await call<Plan>('send_message', {
        id: s.id,
        text,
        options: { ...runOptions(override), mode: runMode },
      });
      if (!override) {
        setDraft('');
        setAttached([]);
      }
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
  async function changeAccessMode(next: AccessMode) {
    setAccessMode(next);
    if (next !== 'read-only' && mode === 'both') setMode('auto');
    if (!session) return;
    try {
      const updated = await call<Session>('set_session_access_mode', {
        id: session.id,
        mode: next,
      });
      activeSession.current = updated;
      setSessions((list) => list.map((item) => (item.id === updated.id ? updated : item)));
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function remove(s: Session) {
    try {
      await call('delete_session', { id: s.id });
      if (selected === s.id) setSelected(null);
      setDeleteTarget(null);
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
  function audit(m: Message) {
    const from = providerOf(m);
    const to = from === 'claude' ? 'gpt' : 'claude';
    if (!accounts.some((a) => a.provider === to)) {
      setError(`Add a ${nameOf(to)} account in Settings to audit this answer.`);
      return;
    }
    void send({
      mode: to,
      auditOf: m.id,
      text: `Audit the previous answer${from ? ` from ${nameOf(from)}` : ''}: find bugs, architectural problems and missing requirements, then propose a corrected version.`,
    });
  }
  async function askRewind(m: Message) {
    if (!session) return;
    try {
      const preview = await call<RewindPreview>('rewind_preview', {
        session: session.id,
        message: m.id,
      });
      setRewindTarget({ id: m.id, preview });
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function doRewind() {
    if (!session || !rewindTarget) return;
    try {
      const text = await call<string>('rewind', { session: session.id, message: rewindTarget.id });
      setRewindTarget(null);
      setMessages(await call<Message[]>('get_messages', { id: session.id }));
      setDraft(text);
      await refresh();
      input.current?.focus();
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function resolveLanes(m: Message, apply: boolean) {
    if (!session) return;
    try {
      await call('resolve_lanes', { session: session.id, message: m.id, apply });
      setMessages(await call<Message[]>('get_messages', { id: session.id }));
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function compact() {
    if (!session) return;
    try {
      const next = await call<Session>('compact_session', { id: session.id });
      await refresh();
      setSelected(next.id);
    } catch (e) {
      setError(errorText(e));
    }
  }
  const historyTokens = messages.reduce((n, m) => n + tokens(m.text), 0);
  const attachedTokens = attached.reduce(
    (n, p) => n + (files.find((f) => f.path === p)?.tokens || 0),
    0,
  );
  const draftTokens = tokens(draft) + attachedTokens;
  const heaviest = attached
    .map((p) => files.find((f) => f.path === p))
    .filter((f): f is FileEntry => !!f)
    .sort((a, b) => b.tokens - a.tokens)[0];
  const heavyFile = !!heaviest && draftTokens > 2000 && heaviest.tokens > draftTokens * 0.8;
  const blob = /[A-Za-z0-9+/=]{4000,}/.test(draft);
  const impact =
    draftTokens + historyTokens > 60_000 ? 'High' : draftTokens > 8000 ? 'Medium' : 'Low';
  const groups: (Message | Message[])[] = [];
  for (const m of messages) {
    const last = groups.at(-1);
    if (laneOf(m)) {
      if (Array.isArray(last)) last.push(m);
      else groups.push([m]);
    } else groups.push(m);
  }
  const renderMessage = (m: Message, inLane = false) => {
    const visible = m.activity.filter((a) => !HIDDEN.includes(a.kind));
    const attachments = m.activity.find((a) => a.kind === 'attachments')?.text.split('\n');
    const lane = m.activity.find((a) => ['worktree', 'applied', 'discarded'].includes(a.kind));
    const from = providerOf(m);
    const other = from === 'claude' ? 'gpt' : 'claude';
    const orchestraState = m.role === 'assistant' ? orchestraOf(m) : null;
    const nogit = m.activity.some((a) => a.kind === 'orchestra-nogit');
    return (
      <article key={m.id} className={`message ${m.role}`}>
        <div className="message-author">
          {m.role === 'assistant' ? (
            <>
              <Mark small />
              <span>{inLane && from ? nameOf(from) : 'Lumen'}</span>
            </>
          ) : (
            <span className="user-mark">You</span>
          )}
          <span className="message-actions">
            {m.role === 'user' && session && !running && (
              <button
                className="icon-button"
                aria-label="Rewind to here"
                title="Rewind to here"
                onClick={() => void askRewind(m)}
              >
                <RotateCcw size={13} />
              </button>
            )}
            {m.role === 'assistant' && m.text && !running && from && (
              <button
                className="text-button audit-button"
                title={`Send to ${nameOf(other)} for audit`}
                onClick={() => audit(m)}
              >
                <Repeat2 size={13} />
                Audit with {nameOf(other)}
              </button>
            )}
            {m.role === 'assistant' && m.text && (
              <button
                className="icon-button"
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
          </span>
        </div>
        {visible.length > 0 && (
          <details className="activity">
            <summary>
              <span className="activity-indicator" />
              {visible.some((a) => a.kind === 'error')
                ? 'Activity & diagnostics'
                : visible.some((a) => a.kind === 'fallback')
                  ? 'Switched provider · activity'
                  : `${visible.length} activity ${visible.length === 1 ? 'event' : 'events'}`}
              <ChevronDown size={13} />
            </summary>
            <div className="activity-content">
              {visible.map((a, i) => (
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
        {orchestraState && (
          <OrchestraCard
            key={`${m.id}-${orchestraState.stage}`}
            message={m}
            state={orchestraState}
            accounts={accounts}
            busy={busy || (running && ['questions', 'plan'].includes(orchestraState.stage))}
            readOnly={accessMode === 'read-only'}
            run={orchestra}
          />
        )}
        {!(orchestraState && ['questions', 'plan'].includes(orchestraState.stage)) && (
          <div className="prose">
            <Suspense fallback={<p>{m.text}</p>}>
              <MessageBody text={m.text} />
            </Suspense>
          </div>
        )}
        {nogit && !running && (
          <p className="orchestra-closed">
            Orchestra needs git in this folder.{' '}
            <button
              className="text-button"
              onClick={() => void orchestra('orchestra_init_git', {})}
            >
              Initialize git
            </button>
          </p>
        )}
        {attachments && (
          <div className="attachment-chips">
            {attachments.map((f) => (
              <span key={f}>
                <Paperclip size={11} /> {f}
              </span>
            ))}
          </div>
        )}
        {lane?.kind === 'worktree' && !running && (
          <button className="primary-button lane-apply" onClick={() => void resolveLanes(m, true)}>
            Apply {from ? nameOf(from) : 'this'} version
          </button>
        )}
        {lane && lane.kind !== 'worktree' && (
          <small className="lane-state">
            {lane.kind === 'applied' ? 'Applied to your project' : 'Discarded'}
          </small>
        )}
        {!m.text && m.role === 'assistant' && running && (
          <span className="working">
            <LoaderCircle size={14} className="spin" />
            Working on it…
          </span>
        )}
      </article>
    );
  };
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
      <aside className={`sidebar ${sidebar ? 'open' : ''}`}>
        <div className="brand">
          <Mark small />
          <span>Lumen</span>
        </div>
        <div className="new-actions">
          <button className="new-chat" onClick={newChat} title="New conversation (Ctrl N)">
            <Plus size={15} />
            <span>New conversation</span>
            <kbd>Ctrl N</kbd>
          </button>
          <button
            className="new-chat"
            onClick={() => setProjectForm(true)}
            title="New project (Ctrl P)"
          >
            <Folder size={15} />
            <span>New project</span>
            <kbd>Ctrl P</kbd>
          </button>
        </div>
        {projectForm && (
          <form
            className="project-form"
            onSubmit={(e) => {
              e.preventDefault();
              void createProject();
            }}
          >
            <input
              autoFocus
              aria-label="Project name"
              placeholder="Project name"
              maxLength={100}
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
            />
            <button type="submit" disabled={!projectName.trim()}>
              Create
            </button>
            <button
              type="button"
              aria-label="Cancel new project"
              onClick={() => setProjectForm(false)}
            >
              <X size={14} />
            </button>
          </form>
        )}
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
        <div className="project-list" aria-label="Projects">
          {[
            { id: '*', name: 'All conversations' },
            ...(workspaces.length ? [{ id: '', name: 'Unsorted' }] : []),
            ...workspaces,
          ].map((w) => (
            <button
              key={w.id}
              className={projectFilter === w.id ? 'selected' : ''}
              onClick={() => {
                setProjectFilter(w.id);
                if (w.id !== '*') {
                  setWorkspaceId(w.id);
                  localStorage.setItem('lumen-workspace-id', w.id);
                }
              }}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                e.preventDefault();
                const s = sessions.find(
                  (s) => s.id === e.dataTransfer.getData('text/lumen-session'),
                );
                if (s && w.id !== '*') void moveSession(s, w.id);
              }}
            >
              <Folder size={13} />
              {w.name}
            </button>
          ))}
        </div>
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
          {sessions
            .filter((s) => projectFilter === '*' || (s.workspaceId || '') === projectFilter)
            .map((s) => (
              <button
                key={s.id}
                draggable
                onDragStart={(e) => {
                  setDraggedSession(s.id);
                  e.dataTransfer.setData('text/lumen-session', s.id);
                }}
                onDragEnd={() => setDraggedSession(null)}
                onDragOver={(e) => {
                  if (draggedSession) e.preventDefault();
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  const id = e.dataTransfer.getData('text/lumen-session');
                  if (id && id !== s.id)
                    void call('reorder_session', { id, beforeId: s.id })
                      .then(refresh)
                      .catch((err) => setError(errorText(err)));
                }}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setContext({
                    session: s,
                    x: Math.min(e.clientX, window.innerWidth - 245),
                    y: Math.min(e.clientY, window.innerHeight - 130),
                  });
                }}
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
          <div className="sidebar-footer-actions">
            <button
              className="usage-button"
              onClick={() => setUsageOpen((open) => !open)}
              aria-expanded={usageOpen}
            >
              <Circle size={8} fill="currentColor" />
              Usage
            </button>
          </div>
        </div>
      </aside>

      {usageOpen && (
        <>
          <button
            className="usage-scrim"
            aria-label="Close usage"
            onClick={() => setUsageOpen(false)}
          />
          <section className="usage-popover" role="dialog" aria-label="Provider usage">
            <header>
              <strong>Usage</strong>
              <button
                className="icon-button"
                aria-label="Refresh usage"
                onClick={() => void loadUsage()}
              >
                <Circle size={14} />
              </button>
              <button
                className="icon-button"
                aria-label="Close usage"
                onClick={() => setUsageOpen(false)}
              >
                <X size={16} />
              </button>
            </header>
            {accounts.length === 0 && <p>No connected providers.</p>}
            {accounts.map((a) => {
              const row = usage.find((u) => u.accountId === a.id);
              const fiveHour = row?.windows.find((w) => w.windowMinutes <= 300);
              const weekly = row?.windows.find((w) => w.windowMinutes >= 10080);
              return (
                <div className="usage-provider" key={a.id}>
                  <strong>
                    {nameOf(a.provider)} <small>{a.name}</small>
                  </strong>
                  <div>{usageBar('5 hour', fiveHour)}</div>
                  <div>{usageBar('Weekly', weekly)}</div>
                </div>
              );
            })}
            <small className="usage-updated">
              Updates every 30 seconds. Unsupported provider quotas show as unavailable.
            </small>
          </section>
        </>
      )}

      {context && (
        <>
          <button
            className="context-scrim"
            aria-label="Close conversation menu"
            onClick={() => setContext(null)}
          />
          <div
            className="popup context-menu"
            role="menu"
            style={{ left: context.x, top: context.y }}
          >
            <button
              role="menuitem"
              onClick={() => {
                setRenameTarget(context.session);
                setRenameValue(context.session.title);
                setContext(null);
              }}
            >
              Rename
            </button>
            <button
              role="menuitem"
              onClick={() => {
                void update(context.session, context.session.title, !context.session.pinned);
                setContext(null);
              }}
            >
              {context.session.pinned ? 'Unpin' : 'Pin conversation'}
            </button>
            {workspaces.length > 0 && (
              <label className="menu-select">
                Move to
                <select
                  aria-label="Move conversation to project"
                  value={context.session.workspaceId || ''}
                  onChange={(e) => void moveSession(context.session, e.target.value)}
                >
                  <option value="">Unsorted</option>
                  {workspaces.map((w) => (
                    <option key={w.id} value={w.id}>
                      {w.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <button
              role="menuitem"
              disabled={active(context.session.status)}
              onClick={() => {
                setDeleteTarget(context.session);
                setContext(null);
              }}
            >
              Delete
            </button>
          </div>
        </>
      )}
      {renameTarget && (
        <div className="dialog-scrim">
          <form
            className="popup conversation-dialog"
            role="dialog"
            aria-label="Rename conversation"
            onSubmit={(e) => {
              e.preventDefault();
              void update(renameTarget, renameValue).then(() => setRenameTarget(null));
            }}
          >
            <label>
              Conversation title
              <input
                autoFocus
                aria-label="Conversation title"
                value={renameValue}
                maxLength={100}
                onChange={(e) => setRenameValue(e.target.value)}
              />
            </label>
            <button disabled={!renameValue.trim()}>Save</button>
            <button type="button" onClick={() => setRenameTarget(null)}>
              Cancel
            </button>
          </form>
        </div>
      )}
      {deleteTarget && (
        <div className="dialog-scrim">
          <div className="popup conversation-dialog" role="dialog" aria-label="Delete conversation">
            <p>Delete "{deleteTarget.title}" and its messages?</p>
            <button onClick={() => void remove(deleteTarget)}>Delete conversation</button>
            <button onClick={() => setDeleteTarget(null)}>Cancel</button>
          </div>
        </div>
      )}
      {rewindTarget && (
        <div className="dialog-scrim">
          <div className="popup conversation-dialog" role="dialog" aria-label="Rewind">
            <p>
              <strong>Rewind to this message?</strong>
              <br />
              {rewindTarget.preview.laterMessages} message
              {rewindTarget.preview.laterMessages === 1 ? '' : 's'} will be removed
              {rewindTarget.preview.restorable
                ? ` and ${rewindTarget.preview.files.length} file${rewindTarget.preview.files.length === 1 ? '' : 's'} restored:`
                : '. No file changes to restore.'}
            </p>
            {rewindTarget.preview.restorable && (
              <ul className="rewind-files">
                {rewindTarget.preview.files.slice(0, 12).map((f) => (
                  <li key={f}>{f}</li>
                ))}
                {rewindTarget.preview.files.length > 12 && (
                  <li>…and {rewindTarget.preview.files.length - 12} more</li>
                )}
              </ul>
            )}
            <button onClick={() => void doRewind()}>Rewind</button>
            <button onClick={() => setRewindTarget(null)}>Cancel</button>
          </div>
        </div>
      )}
      <main className="workspace">
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
              {namedWorkspace?.name ||
                (currentWorkspace
                  ? currentWorkspace.split(/[\\/]/).filter(Boolean).at(-1)
                  : 'Personal workspace')}
            </span>
            <span className="topbar-divider" />
            <span className="conversation-label">{session?.title || 'New conversation'}</span>
          </div>
          <div className="topbar-actions">
            {!desktop && <span className="preview-label">Browser preview</span>}
            {(['claude', 'gpt'] as const)
              .filter((p) => accounts.some((a) => a.provider === p))
              .map((p) => {
                const rows = health.filter((h) =>
                  accounts.some((a) => a.id === h.accountId && a.provider === p),
                );
                const best =
                  rows.find((h) => h.state === 'active') ||
                  rows.find((h) => h.state === 'near') ||
                  rows[0];
                const state = best?.state || 'active';
                return (
                  <span
                    key={p}
                    className={`health-pill ${state}`}
                    title={
                      state === 'cooldown'
                        ? `${nameOf(p)} is cooling down after a rate limit`
                        : state === 'near'
                          ? `${nameOf(p)} is close to its usage limit`
                          : `${nameOf(p)} is ready · ${best?.turns || 0} turns in the last 5 hours`
                    }
                  >
                    <span className={`health-dot ${state}`} />
                    {nameOf(p)}
                    {state === 'cooldown' && best?.until ? ` · ${countdown(best.until)}` : ''}
                  </span>
                );
              })}
            <button
              className={`icon-button ${projectOpen ? 'on' : ''}`}
              aria-label="Project context"
              title="Project context: shared notes and files"
              aria-pressed={projectOpen}
              onClick={() => setProjectOpen(!projectOpen)}
            >
              <BookOpen size={16} />
            </button>
            <button className="icon-button" aria-label="Settings" onClick={() => openSettings()}>
              <Settings2 size={17} />
            </button>
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
            </div>
          ) : (
            <div className="message-list">
              {groups.map((g) =>
                Array.isArray(g) ? (
                  <div className="lanes" key={g[0].id}>
                    <div className="lane-grid">{g.map((m) => renderMessage(m, true))}</div>
                    {g.some((m) => m.activity.some((a) => a.kind === 'worktree')) && !running && (
                      <button
                        className="text-button lane-discard"
                        onClick={() => {
                          const m = g.find((x) => x.activity.some((a) => a.kind === 'worktree'));
                          if (m) void resolveLanes(m, false);
                        }}
                      >
                        Discard both versions
                      </button>
                    )}
                  </div>
                ) : (
                  renderMessage(g)
                ),
              )}
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
          {session && historyTokens > 40_000 && !running && (
            <button className="compact-chip" onClick={() => void compact()}>
              <Shrink size={13} />
              Long conversation · Compact and continue in a fresh session
            </button>
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
                    : plan.strategy === 'broadcast'
                      ? 'Claude and ChatGPT side by side'
                      : plan.strategy === 'orchestra'
                        ? 'Orchestra'
                        : 'Answer + cross-audit'}{' '}
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
            {(mode === 'claude' || mode === 'gpt') && (
              <ProviderOptions
                provider={mode}
                visible={advanced}
                accounts={accounts}
                accountId={accountId}
                setAccountId={setAccountId}
                model={model}
                setModel={setModel}
                effort={effort}
                setEffort={setEffort}
                manage={() => openSettings('accounts')}
                refreshAccounts={loadAccounts}
                onError={setError}
                onReady={setCatalogReady}
                paused={settings}
              />
            )}
            <div className="composer-toolbar">
              <div className="mode-control" role="group" aria-label="AI provider">
                {(accessMode === 'read-only'
                  ? (['auto', 'claude', 'gpt', 'both'] as const)
                  : (['auto', 'claude', 'gpt'] as const)
                ).map((p) => (
                  <button
                    key={p}
                    aria-pressed={mode === p}
                    className={mode === p ? 'active' : ''}
                    onClick={() => {
                      setMode(p);
                      setAccountId('');
                      setModel('');
                      setEffort('');
                      setCatalogReady(false);
                      setAdvanced(false);
                    }}
                  >
                    {p === 'auto' && <Sparkles size={13} />}
                    {p === 'both' && <Columns2 size={13} />}
                    <span>{p === 'auto' ? 'Auto' : p === 'both' ? 'Both' : nameOf(p)}</span>
                  </button>
                ))}
              </div>
              {mode === 'auto' && appSettings?.orchestra && (
                <button
                  className="orchestra-badge"
                  title="Orchestra is on: complex tasks are planned and split across agents. Click to turn off."
                  onClick={() =>
                    void saveSettings({ ...appSettings, orchestra: false }).catch((e) =>
                      setError(errorText(e)),
                    )
                  }
                >
                  <Workflow size={12} />
                  Orchestra
                </button>
              )}
              {mode !== 'both' && (
                <button
                  className={`icon-button audit-toggle ${autoReview ? 'on' : ''}`}
                  aria-label="Cross-audit"
                  aria-pressed={autoReview}
                  title={
                    autoReview
                      ? 'Cross-audit on: the other model reviews every answer'
                      : 'Cross-audit off'
                  }
                  onClick={() => {
                    setAutoReview(!autoReview);
                    localStorage.setItem('lumen-auto-review', String(!autoReview));
                  }}
                >
                  <Repeat2 size={15} />
                </button>
              )}
              <div className="workspace-controls">
                <select
                  aria-label="Project"
                  title={namedWorkspace?.name || 'Choose project'}
                  value={currentWorkspaceId}
                  disabled={running || folderBusy}
                  onChange={(e) => void chooseWorkspace(e.target.value)}
                >
                  {workspaces.map((w) => (
                    <option key={w.id} value={w.id}>
                      {w.name}
                    </option>
                  ))}
                  <option value="__add">New project…</option>
                </select>
                <button
                  className="folder-control"
                  aria-label="Choose conversation folder"
                  title={currentWorkspace || 'Choose conversation folder'}
                  disabled={running || folderBusy}
                  onClick={() => void chooseFolder().catch((e) => setError(errorText(e)))}
                >
                  <Folder size={14} />
                  <span>
                    {currentWorkspace
                      ? currentWorkspace.split(/[\\/]/).filter(Boolean).at(-1)
                      : 'Choose folder'}
                  </span>
                </button>
              </div>
              <button
                className={`icon-button attach-button ${attached.length ? 'on' : ''}`}
                aria-label="Attach files"
                title={attached.length ? `${attached.length} files attached` : 'Attach files'}
                onClick={() => setProjectOpen(true)}
              >
                <Paperclip size={15} />
                {attached.length > 0 && <small>{attached.length}</small>}
              </button>
              {(mode === 'claude' || mode === 'gpt') && (
                <button
                  className={`icon-button advanced-toggle ${advanced ? 'on' : ''}`}
                  aria-label="Advanced provider options"
                  aria-expanded={advanced}
                  onClick={() => setAdvanced(!advanced)}
                >
                  {advanced ? <ChevronDown size={15} /> : <ChevronUp size={15} />}
                </button>
              )}
              <div className="composer-spacer" />
              <div className={`access-control ${accessMode === 'autopilot' ? 'autopilot' : ''}`}>
                <ShieldCheck size={13} />
                <select
                  aria-label="Permissions"
                  value={accessMode}
                  title={
                    accessMode === 'autopilot'
                      ? 'Full access: commands, network, files and configured automation tools. No CLI approval prompts.'
                      : 'Conversation permissions'
                  }
                  onChange={(e) => void changeAccessMode(e.target.value as AccessMode)}
                >
                  <option value="read-only">Ask</option>
                  <option value="file-edits">File edits on</option>
                  <option value="autopilot">Autopilot</option>
                </select>
              </div>
              <button
                className="send-button"
                aria-label={running ? 'Stop response' : 'Send message'}
                disabled={
                  !running &&
                  (!draft.trim() ||
                    folderBusy ||
                    (desktop && (mode === 'claude' || mode === 'gpt') && !catalogReady))
                }
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
            {draft.trim() && (
              <span
                className={`token-badge ${heavyFile || blob ? 'warn' : impact.toLowerCase()}`}
                title={
                  heavyFile
                    ? `${heaviest.path} is more than 80% of this prompt`
                    : blob
                      ? 'This message contains a large encoded blob'
                      : `Estimated prompt size · quota impact ${impact}`
                }
              >
                ~{compactTokens(draftTokens)} tokens · {heavyFile || blob ? 'Heavy' : impact}
              </span>
            )}
            <span>
              Enter to send <span className="caption-dot">·</span> Shift Enter for a new line
            </span>
          </div>
        </div>
      </main>
      {projectOpen && (
        <ProjectPanel
          folder={currentWorkspace}
          attached={attached}
          setAttached={setAttached}
          onFiles={setFiles}
          onClose={() => setProjectOpen(false)}
          onError={setError}
        />
      )}

      <QuitConfirm />
      <SettingsPanel
        open={settings}
        tab={settingsTab}
        onClose={() => setSettings(false)}
        error={error}
        setError={setError}
        accounts={accounts}
        refreshAccounts={loadAccounts}
        initialProvider={mode === 'gpt' ? 'gpt' : 'claude'}
        autoPolicy={autoPolicy}
        setAutoPolicy={setAutoPolicy}
        balanceUsage={balanceUsage}
        setBalanceUsage={setBalanceUsage}
        theme={theme}
        setTheme={setTheme}
        workspace={workspace}
        chooseFolder={chooseFolder}
      />
    </div>
  );
}
