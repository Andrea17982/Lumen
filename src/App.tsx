import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import type { ClipboardEvent } from 'react';
import { useReducedMotion } from 'framer-motion';
import {
  ArrowUp,
  Check,
  ChevronDown,
  ChevronUp,
  Copy,
  Folder,
  MoreHorizontal,
  MessageSquare,
  Pin,
  Plus,
  Search,
  Sparkles,
  Square,
  X,
  AlertCircle,
  Circle,
  RotateCcw,
  Paperclip,
  Repeat2,
  Shrink,
  Workflow,
  MapPin,
} from 'lucide-react';
import ProviderOptions from './components/ProviderOptions';
import QuitConfirm from './components/QuitConfirm';
import OrchestraCard from './components/OrchestraCard';
import { saveSettings, useAppSettings } from './lib/settings';
import SettingsPanel, { type SettingsTab } from './settings/SettingsPanel';
import {
  applyEvent,
  approvalSnapshot,
  call,
  decideApproval,
  desktop,
  onApprovalChange,
  onUsage,
  subscribe,
  type ApprovalSnapshot,
} from './lib/bridge';
import { GatekeeperPanel } from './features/gatekeeper';
import { formatResetCountdown } from './lib/usage';
import { localAssistant } from './features/local-assistant/instance';
import { nameNewChat } from './features/local-assistant/autoTitle';
import { splitReasoning, workPhase } from './lib/reasoning';
import { Draft } from './features/local-assistant/draft';
import DraftSheet from './components/DraftSheet';
import WorkingIndicator from './components/WorkingIndicator';
import AttachmentPreview from './components/AttachmentPreview';
import Welcome from './components/Welcome';
import Topbar, { type HealthPill } from './components/Topbar';
import ModeControl from './components/ModeControl';
import PermissionButton, { ACCESS_ORDER } from './components/PermissionButton';
import UsagePopover from './components/UsagePopover';
import ContextMeter from './components/ContextMeter';
import CommandPalette from './components/CommandPalette';
import { runCommand, type Command } from './lib/commands';
import {
  contextWindow,
  estimateTokens,
  lastMeasurement,
  contextUse as measureContext,
  tokensForChars,
} from './lib/context';
import type {
  Account,
  AccessMode,
  Workspace,
  Message,
  Mode,
  Plan,
  AccountUsage,
  Health,
  RewindPreview,
  RunEvent,
  Session,
  Status,
  OrchestraState,
} from './lib/types';
import { errorText, IconButton, Mark, nameOf } from './lib/util';
import { useI18n } from './lib/i18n';

const MessageBody = lazy(() => import('./components/MessageBody'));
const active = (s?: Status) => s === 'running' || s === 'waiting';
const HIDDEN = [
  'thinking',
  'checkpoint',
  'lane',
  'worktree',
  'applied',
  'discarded',
  'attachments',
  'orchestra',
  'orchestra-nogit',
  'usage',
  'references',
  'native',
  'resume',
  'route',
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
const tokens = estimateTokens;
const compactTokens = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n));
function mergeUsage(current: AccountUsage[], incoming: AccountUsage[]): AccountUsage[] {
  const merged = new Map(current.map((row) => [row.accountId, row]));
  for (const row of incoming) {
    const previous = merged.get(row.accountId);
    if (!previous || row.updatedAt >= previous.updatedAt) merged.set(row.accountId, row);
  }
  return [...merged.values()];
}

export default function App() {
  const { t } = useI18n();
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
  const contextTrigger = useRef<HTMLElement | null>(null);
  const [renameTarget, setRenameTarget] = useState<Session | null>(null);
  const [renamePos, setRenamePos] = useState({ x: 0, y: 0 });
  const [usagePos, setUsagePos] = useState({ left: 276, bottom: 74 });
  const [deleteTarget, setDeleteTarget] = useState<Session | null>(null);
  const [agyNotice, setAgyNotice] = useState(false);
  const [agyHide, setAgyHide] = useState(false);
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
  const [usageClock, setUsageClock] = useState(Date.now());
  const [usageErrors, setUsageErrors] = useState<Record<string, string>>({});
  const usageRequests = useRef(new Map<string, Promise<void>>());
  useEffect(() => {
    const timer = window.setInterval(() => setUsageClock(Date.now()), 15_000);
    return () => window.clearInterval(timer);
  }, []);
  useEffect(() => {
    const now = Date.now();
    const nextReset = Math.min(
      ...usage
        .flatMap((row) => row.windows.map((w) => w.resetsAt))
        .filter((time) => Number.isFinite(time) && time > now),
    );
    if (!Number.isFinite(nextReset)) return;
    // Browser timers cannot represent delays beyond approximately 24 days.
    const timer = window.setTimeout(
      () => setUsageClock(Date.now()),
      Math.min(nextReset - now + 1, 2_147_483_647),
    );
    return () => window.clearTimeout(timer);
  }, [usage, usageClock]);
  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void onUsage((row) => {
      setUsage((old) => mergeUsage(old, [row]));
    }).then((stop) => {
      if (disposed) stop();
      else unsubscribe = stop;
    });
    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);
  const [usageOpen, setUsageOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [theme, setTheme] = useState(() => localStorage.getItem('lumen-theme') || 'system');
  const [workspace, setWorkspace] = useState(() => localStorage.getItem('lumen-workspace') || '');
  const [copyState, setCopyState] = useState('');
  const [sheet, setSheet] = useState<{ kind: 'commit' | 'handoff'; draft: Draft } | null>(null);
  const [autoReview] = useState(() => localStorage.getItem('lumen-auto-review') === 'true');
  const [attached, setAttached] = useState<string[]>([]);
  const appSettings = useAppSettings();
  useEffect(() => {
    if (appSettings) localAssistant.configure(appSettings.localAssistant);
  }, [appSettings]);
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
  // The backend sizes the prompt the next turn would send. It refreshes when a run ends, the
  // chat changes or its permission mode changes; until then the meter uses visible text.
  const [promptEstimate, setPromptEstimate] = useState<{ id: string; chars: number } | null>(null);
  const sessionId = session?.id;
  const sessionAccess = session?.accessMode;
  useEffect(() => {
    if (!sessionId || running) return;
    let gone = false;
    void call<number>('prompt_estimate', { id: sessionId })
      .then((chars) => {
        if (!gone) setPromptEstimate({ id: sessionId, chars });
      })
      .catch(() => {
        if (!gone) setPromptEstimate(null);
      });
    return () => {
      gone = true;
    };
  }, [sessionId, sessionAccess, running]);
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
    const saved = await call<AccountUsage[]>('get_usage');
    setUsage((old) => mergeUsage(old, saved));
    await Promise.all(
      accounts
        .filter((a) => ['gpt', 'antigravity'].includes(a.provider))
        .map((a) => {
          const pending = usageRequests.current.get(a.id);
          if (pending) return pending;
          const request = (async () => {
            try {
              const row = await call<AccountUsage>('refresh_usage', { id: a.id });
              setUsage((old) => mergeUsage(old, [row]));
              setUsageErrors((old) => ({ ...old, [a.id]: '' }));
            } catch (error) {
              setUsageErrors((old) => ({ ...old, [a.id]: errorText(error) }));
            }
          })().finally(() => usageRequests.current.delete(a.id));
          usageRequests.current.set(a.id, request);
          return request;
        }),
    );
  }, [accounts]);
  useEffect(() => {
    const refreshUsage = () => void loadUsage().catch(() => {});
    refreshUsage();
    const timer = window.setInterval(refreshUsage, 30_000);
    return () => window.clearInterval(timer);
  }, [loadUsage]);
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
        const chat = activeSession.current;
        if (chat?.id === event.sessionId)
          void call<Message[]>('get_messages', { id: chat.id })
            .then((m) => nameNewChat(localAssistant, chat, m, () => activeSession.current?.title))
            .then((title) => {
              if (!title) return;
              if (activeSession.current?.id === chat.id)
                activeSession.current = { ...activeSession.current, title };
              return refreshRef.current();
            })
            .catch(() => {
              // The helper is optional: a failed title keeps the derived one.
            });
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
  // Shortcuts run registry commands through this ref, so the window listener sees current state.
  const commandsRef = useRef<Command[]>([]);
  useEffect(() => {
    const fn = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'p') {
        e.preventDefault();
        runCommand(commandsRef.current, 'new-project');
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
        runCommand(commandsRef.current, 'new-chat');
      }
      if (
        e.key === '/' &&
        !(
          e.target instanceof HTMLElement &&
          (['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName) || e.target.isContentEditable)
        )
      ) {
        e.preventDefault();
        runCommand(commandsRef.current, 'search');
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
  async function attachFiles() {
    if (!desktop) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const paths = await open({ multiple: true, directory: false, title: 'Attach files' });
      if (Array.isArray(paths))
        setAttached((current) => [...new Set([...current, ...paths])].slice(0, 50));
      else if (typeof paths === 'string')
        setAttached((current) => [...new Set([...current, paths])].slice(0, 50));
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function pasteImage(e: ClipboardEvent<HTMLTextAreaElement>) {
    const item = Array.from(e.clipboardData.items).find((entry) => entry.type.startsWith('image/'));
    const file = item?.getAsFile();
    if (!file) return;
    e.preventDefault();
    if (!desktop) {
      setError('Paste images in the Lumen desktop app.');
      return;
    }
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      let binary = '';
      for (let offset = 0; offset < bytes.length; offset += 0x8000)
        binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
      const path = await call<string>('save_pasted_image', {
        data: btoa(binary),
        mime: file.type || 'image/png',
      });
      setAttached((current) => [...new Set([...current, path])].slice(0, 50));
    } catch (error) {
      setError(errorText(error));
    }
  }
  async function chooseWorkspace(id: string) {
    setFolderBusy(true);
    try {
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
        options: { ...runOptions(override), mode: runMode, attachments: override ? [] : attached },
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
    if (
      next === 'file-edits' &&
      accounts.some((a) => a.provider === 'antigravity') &&
      localStorage.getItem('lumen-agy-file-edits-notice') !== 'hidden'
    )
      setAgyNotice(true);
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
  async function compact(summary?: string) {
    if (!session) return;
    try {
      const next = await call<Session>('compact_session', { id: session.id, summary });
      await refresh();
      setSelected(next.id);
      setSheet(null);
    } catch (e) {
      setError(errorText(e));
    }
  }
  // A draft belongs to the chat it was opened in.
  useEffect(() => {
    setSheet(null);
  }, [session?.id]);
  // Approvals are per chat; the native journal pushes every change, so reload on each one.
  const [approvals, setApprovals] = useState<ApprovalSnapshot>({ requests: [], sessions: [] });
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | undefined;
    const load = () => {
      if (!session) return;
      approvalSnapshot(session.id)
        .then((next) => alive && setApprovals(next))
        .catch(() => undefined);
    };
    setApprovals({ requests: [], sessions: [] });
    load();
    void onApprovalChange(load).then((off) => {
      if (alive) stop = off;
      else off();
    });
    return () => {
      alive = false;
      stop?.();
    };
  }, [session?.id]);
  /** Opens the handoff draft; the conversation text is captured now, generation stays on request. */
  function openHandoff() {
    const transcript = messages
      .map((m) => `${m.role === 'user' ? 'User' : 'Assistant'}: ${m.text}`)
      .join('\n\n')
      .slice(-30_000);
    setSheet({ kind: 'handoff', draft: Draft.handoff(localAssistant, () => transcript) });
  }
  async function openCommit() {
    if (!session) return;
    try {
      const diff = await call<string>('working_diff', { id: session.id });
      if (!diff.trim()) {
        setError('There are no uncommitted changes to describe.');
        return;
      }
      setSheet({ kind: 'commit', draft: Draft.commit(localAssistant, () => diff) });
    } catch (e) {
      setError(errorText(e));
    }
  }
  const historyTokens = messages.reduce((n, m) => n + tokens(m.text), 0);
  const attachedTokens = 0; // Arbitrary external files are not read merely to estimate context.
  const draftTokens = tokens(draft) + attachedTokens;
  const contextProvider =
    mode !== 'auto' && mode !== 'both'
      ? mode
      : (accounts.find((a) => a.id === accountId)?.provider ??
        plan?.provider ??
        // Only certain when every connected account belongs to the same provider.
        (accounts.length > 0 && accounts.every((a) => a.provider === accounts[0].provider)
          ? accounts[0].provider
          : null));
  const promptTokens =
    promptEstimate && promptEstimate.id === session?.id
      ? tokensForChars(promptEstimate.chars)
      : historyTokens;
  // Only Claude reports its window and CLI overhead; other providers stay a local estimate.
  const contextUse = contextProvider
    ? measureContext(
        promptTokens,
        draftTokens,
        contextWindow(contextProvider, model || plan?.model || ''),
        contextProvider === 'claude' ? lastMeasurement(messages) : null,
      )
    : null;
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
    const inline =
      m.role === 'assistant' ? splitReasoning(m.text) : { reasoning: '', answer: m.text };
    const reasoning = [
      ...m.activity.filter((a) => a.kind === 'thinking').map((a) => a.text),
      inline.reasoning,
    ]
      .filter(Boolean)
      .join('\n\n');
    const isLive = running && m.role === 'assistant' && m.id === messages.at(-1)?.id;
    const attachments = m.activity.find((a) => a.kind === 'attachments')?.text.split('\n');
    const references = m.activity.find((a) => a.kind === 'references')?.text.split('\n');
    const lane = m.activity.find((a) => ['worktree', 'applied', 'discarded'].includes(a.kind));
    const from = providerOf(m);
    const other = from === 'claude' ? 'gpt' : 'claude';
    const orchestraState = m.role === 'assistant' ? orchestraOf(m) : null;
    const nogit = m.activity.some((a) => a.kind === 'orchestra-nogit');
    return (
      <article
        key={m.id}
        className={`message ${m.role}${m.id === messages.at(-1)?.id ? ' is-latest' : ''}`}
      >
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
                    .writeText(inline.answer)
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
        {reasoning && (
          <details className="reasoning">
            <summary>
              <ChevronDown size={13} />
              Reasoning
            </summary>
            <pre>{reasoning}</pre>
          </details>
        )}
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
                    {a.kind === 'file'
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
            <Suspense fallback={<p>{inline.answer}</p>}>
              <MessageBody text={inline.answer} />
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
                <Paperclip size={11} /> {f.split(/[\\/]/).filter(Boolean).at(-1) || f}
                {/\.(png|jpe?g|webp|gif)$/i.test(f) && session?.workspace && (
                  <AttachmentPreview folder={session.workspace} path={f} />
                )}
              </span>
            ))}
          </div>
        )}
        {references && (
          <div
            className="attachment-chips"
            aria-label="Repository map locations added to this request"
            title="Added to the prompt from the repository map"
          >
            {references.map((r) => (
              <span key={r}>
                <MapPin size={11} /> {r}
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
        {isLive && <WorkingIndicator phase={workPhase(m.activity.map((a) => a.kind))} />}
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

  /** Actions on one conversation; the sidebar menu and the palette run the same entries. */
  const sessionCommands = (
    s: Session,
    at = { x: window.innerWidth / 2 - 136, y: 96 },
  ): Command[] => [
    {
      id: 'session-rename',
      label: 'Rename',
      group: 'Conversation',
      run: () => {
        setRenameTarget(s);
        setRenameValue(s.title);
        setRenamePos({
          x: Math.min(at.x, window.innerWidth - 272),
          y: Math.min(at.y, window.innerHeight - 150),
        });
      },
    },
    {
      id: 'session-pin',
      label: s.pinned ? 'Unpin' : 'Pin conversation',
      group: 'Conversation',
      run: () => void update(s, s.title, !s.pinned),
    },
    {
      id: 'session-delete',
      label: 'Delete',
      group: 'Conversation',
      available: !active(s.status),
      run: () => setDeleteTarget(s),
    },
  ];

  const commands: Command[] = [
    {
      id: 'new-chat',
      label: 'New conversation',
      group: 'Conversation',
      shortcut: 'Ctrl N',
      run: newChat,
    },
    {
      id: 'new-project',
      label: 'New project',
      group: 'Project',
      shortcut: 'Ctrl P',
      run: () => {
        setSidebar(true);
        setProjectForm(true);
      },
    },
    {
      id: 'search',
      label: 'Search conversations',
      group: 'Conversation',
      shortcut: '/',
      run: () => {
        setSidebar(true);
        requestAnimationFrame(() => searchInput.current?.focus());
      },
    },
    {
      id: 'compact',
      label: 'Compact context into a fresh session',
      group: 'Conversation',
      keywords: 'handoff summary shrink',
      available: !!session && !running && messages.length > 0,
      run: () => void compact(),
    },
    {
      id: 'handoff',
      label: 'Draft a handoff summary',
      group: 'Conversation',
      available: !!session && !running && messages.length > 0,
      run: openHandoff,
    },
    {
      id: 'commit',
      label: 'Draft a commit message',
      group: 'Project',
      keywords: 'git diff',
      available: !!session && !running,
      run: () => void openCommit(),
    },
    {
      id: 'usage',
      label: 'Show provider usage',
      group: 'Providers',
      keywords: 'quota limits',
      run: () => setUsageOpen(true),
    },
    ...(['general', 'accounts', 'auto', 'teamwork', 'tools', 'about'] as SettingsTab[]).map(
      (tab): Command => ({
        id: `settings-${tab}`,
        label: `Settings: ${tab[0].toUpperCase()}${tab.slice(1)}`,
        group: 'Settings',
        keywords: 'preferences',
        run: () => openSettings(tab),
      }),
    ),
    ...ACCESS_ORDER.map((access): Command => ({
      id: `access-${access}`,
      label: `Permission mode: ${{ 'read-only': 'Ask (read-only)', 'file-edits': 'Edits', ask: 'Approve each action', autopilot: 'Autopilot' }[access]}`,
      group: 'Permissions',
      keywords: 'access',
      run: () => void changeAccessMode(access),
    })),
    ...(['system', 'light', 'dark'] as const).map((name): Command => ({
      id: `theme-${name}`,
      label: `Theme: ${name}`,
      group: 'Appearance',
      run: () => setTheme(name),
    })),
    ...(session ? sessionCommands(session) : []),
  ];
  useEffect(() => {
    commandsRef.current = commands;
  });

  const providerPills: HealthPill[] = (['claude', 'gpt'] as const)
    .filter((p) => accounts.some((a) => a.provider === p))
    .map((p) => {
      const rows = health.filter((h) =>
        accounts.some((a) => a.id === h.accountId && a.provider === p),
      );
      const best =
        rows.find((h) => h.state === 'active') || rows.find((h) => h.state === 'near') || rows[0];
      const state = best?.state || 'active';
      return {
        key: p,
        state,
        label:
          state === 'cooldown' && best?.until
            ? `${nameOf(p)} · ${formatResetCountdown(best.until)}`
            : nameOf(p),
        title:
          state === 'cooldown'
            ? `${nameOf(p)} is cooling down after a rate limit`
            : state === 'near'
              ? `${nameOf(p)} is close to its usage limit`
              : `${nameOf(p)} is ready · ${best?.turns || 0} turns in the last 5 hours`,
      };
    });

  return (
    <div className="app-shell">
      {paletteOpen && <CommandPalette commands={commands} onClose={() => setPaletteOpen(false)} />}
      {sidebar && (
        <button
          className="sidebar-scrim"
          aria-label={t('Close sidebar')}
          onClick={() => setSidebar(false)}
        />
      )}
      <aside className={`sidebar ${sidebar ? 'open' : ''}`}>
        <div className="brand">
          <span className="brand-tile">
            <Mark small />
          </span>
          <span>Lumen</span>
        </div>
        <div className="new-actions">
          <button
            className="new-chat"
            onClick={newChat}
            title={`${t('New conversation')} (Ctrl N)`}
          >
            <Plus size={15} />
            <span>{t('New conversation')}</span>
            <kbd>Ctrl N</kbd>
          </button>
          <button
            className="new-chat"
            onClick={() => setProjectForm(true)}
            title={`${t('New project')} (Ctrl P)`}
          >
            <Folder size={15} />
            <span>{t('New project')}</span>
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
              aria-label={t('Project name')}
              placeholder={t('Project name')}
              maxLength={100}
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
            />
            <button type="submit" disabled={!projectName.trim()}>
              {t('Create')}
            </button>
            <button
              type="button"
              aria-label={t('Cancel new project')}
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
            aria-label={t('Search conversations')}
            placeholder={t('Search conversations')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <kbd>/</kbd>
        </label>
        <div className="project-list" aria-label="Projects">
          {[
            { id: '*', name: t('All conversations') },
            ...(workspaces.length ? [{ id: '', name: t('Unsorted') }] : []),
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
          <span>{t('Your conversations')}</span>
          <span>
            {sessions.length}
            {hasMore ? '+' : ''}
          </span>
        </div>
        <nav className="session-list" aria-label="Conversations">
          {sessions.length === 0 && (
            <div className="sidebar-empty">
              <MessageSquare size={20} />
              <p>
                {search ? t('No conversations found.') : t('A little room for your next big idea.')}
              </p>
            </div>
          )}
          {sessions
            .filter((s) => projectFilter === '*' || (s.workspaceId || '') === projectFilter)
            .map((s) => (
              <div className="session-item" key={s.id}>
                <button
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
                    contextTrigger.current = e.currentTarget;
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
                <IconButton
                  className="session-menu-button"
                  label={`Actions for ${s.title}`}
                  aria-haspopup="menu"
                  aria-expanded={context?.session.id === s.id}
                  onClick={(e) => {
                    contextTrigger.current = e.currentTarget;
                    const rect = e.currentTarget.getBoundingClientRect();
                    setContext({
                      session: s,
                      x: Math.max(0, Math.min(rect.right, window.innerWidth - 245)),
                      y: Math.max(0, Math.min(rect.bottom, window.innerHeight - 180)),
                    });
                  }}
                >
                  <MoreHorizontal size={16} />
                </IconButton>
              </div>
            ))}
          {hasMore && (
            <button className="text-button load-more" onClick={() => void loadMore()}>
              {t('Load more conversations')}
            </button>
          )}
        </nav>
        <div className="sidebar-bottom">
          <div className="sidebar-footer-actions">
            <button
              className="usage-button"
              onClick={(e) => {
                // Open beside the button, whatever the sidebar width.
                const r = e.currentTarget.getBoundingClientRect();
                setUsagePos({
                  left: Math.max(12, Math.min(r.right + 10, window.innerWidth - 352)),
                  bottom: Math.max(12, window.innerHeight - r.bottom),
                });
                setUsageOpen((open) => !open);
              }}
              aria-expanded={usageOpen}
            >
              <Circle size={8} fill="currentColor" />
              {t('Usage')}
            </button>
            {session && (
              <ContextMeter
                use={contextUse}
                disabled={running || messages.length === 0}
                onCompact={() => void compact()}
              />
            )}
          </div>
        </div>
      </aside>

      {usageOpen && (
        <UsagePopover
          accounts={accounts}
          usage={usage}
          errors={usageErrors}
          now={usageClock}
          position={usagePos}
          onRefresh={() => void loadUsage()}
          onClose={() => setUsageOpen(false)}
        />
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
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                e.preventDefault();
                setContext(null);
                contextTrigger.current?.focus();
              }
            }}
          >
            {(() => {
              const [rename, pin, remove] = sessionCommands(context.session, context);
              const item = (c: Command, autoFocus = false) => (
                <button
                  key={c.id}
                  role="menuitem"
                  autoFocus={autoFocus}
                  disabled={c.available === false}
                  onClick={() => {
                    c.run();
                    setContext(null);
                  }}
                >
                  {c.label}
                </button>
              );
              return (
                <>
                  {item(rename, true)}
                  {item(pin)}
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
                  {item(remove)}
                </>
              );
            })()}
          </div>
        </>
      )}
      {renameTarget && (
        <>
          <button
            className="context-scrim"
            aria-label="Close rename"
            onClick={() => setRenameTarget(null)}
          />
          <form
            className="popup conversation-dialog rename-popover"
            role="dialog"
            aria-label="Rename conversation"
            style={{ left: renamePos.x, top: renamePos.y }}
            onKeyDown={(e) => e.key === 'Escape' && setRenameTarget(null)}
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
            <div className="rename-actions">
              <button type="button" onClick={() => setRenameTarget(null)}>
                Cancel
              </button>
              <button disabled={!renameValue.trim()}>Save</button>
            </div>
          </form>
        </>
      )}
      {agyNotice && (
        <div className="dialog-scrim">
          <div
            className="popup conversation-dialog notice-dialog"
            role="dialog"
            aria-label="Antigravity in File edits"
          >
            <strong>Antigravity can run any command</strong>
            <p>
              In File edits, Claude and ChatGPT can only edit files. Antigravity's command line
              can't run even read-only commands without permission, so Lumen lets it run every
              command without asking. It only runs as a fallback or for Orchestra subtasks.
            </p>
            <label className="notice-check">
              <input
                type="checkbox"
                checked={agyHide}
                onChange={(e) => setAgyHide(e.target.checked)}
              />
              Don't show again
            </label>
            <button
              autoFocus
              onClick={() => {
                if (agyHide) localStorage.setItem('lumen-agy-file-edits-notice', 'hidden');
                setAgyNotice(false);
              }}
            >
              OK
            </button>
          </div>
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
        <div className="aurora" aria-hidden="true" />
        <Topbar
          desktop={desktop}
          workspaceLabel={
            namedWorkspace?.name ||
            (currentWorkspace
              ? currentWorkspace.split(/[\\/]/).filter(Boolean).at(-1) || ''
              : t('Personal workspace'))
          }
          workspaceTitle={currentWorkspace || undefined}
          conversationTitle={session?.title || t('New conversation')}
          pills={providerPills}
          onChooseFolder={!session ? () => void chooseFolder() : undefined}
          folderBusy={folderBusy || running}
          onOpenSidebar={() => setSidebar(true)}
          onOpenSettings={() => openSettings()}
        />
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
            <Welcome reduced={!!reduced} />
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
                aria-label={t('Dismiss error')}
                onClick={() => setError('')}
              >
                <X size={14} />
              </button>
            </div>
          )}
          {session && historyTokens > 40_000 && !running && (
            <button className="compact-chip" onClick={openHandoff}>
              <Shrink size={13} />
              Long conversation · Draft a handoff and continue in a fresh session
            </button>
          )}
          {session && !running && !sheet && (
            <button className="compact-chip" onClick={() => void openCommit()}>
              <Sparkles size={13} />
              Draft a commit message for uncommitted changes
            </button>
          )}
          {session &&
            approvals.requests.some(
              (r) => r.status === 'pending' || r.delivery === 'uncertain',
            ) && (
              <GatekeeperPanel
                requests={approvals.requests.filter(
                  (r) => r.status === 'pending' || r.delivery === 'uncertain',
                )}
                sessions={approvals.sessions}
                onDecide={decideApproval}
              />
            )}
          {session && sheet && (
            <DraftSheet
              key={sheet.kind}
              title={sheet.kind === 'commit' ? 'Commit message draft' : 'Handoff draft'}
              draft={sheet.draft}
              label={sheet.kind === 'commit' ? 'Commit message' : 'Handoff summary'}
              useLabel={sheet.kind === 'commit' ? 'Copy message' : 'Continue in a fresh session'}
              onClose={() => setSheet(null)}
              onUse={(text) => {
                if (sheet.kind === 'commit') {
                  void navigator.clipboard
                    .writeText(text)
                    .then(() => setSheet(null))
                    .catch((e) => setError(errorText(e)));
                } else void compact(text);
              }}
              footer={
                sheet.kind === 'handoff' ? (
                  <button className="text-button" onClick={() => void compact()}>
                    Continue with the built-in summary
                  </button>
                ) : null
              }
            />
          )}
          {plan && (
            <div className="plan-line">
              <Sparkles size={13} />
              <span>
                {nameOf(plan.provider)} ·{' '}
                {plan.strategy === 'single'
                  ? 'Direct'
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
              placeholder={t("What's on your mind?")}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onPaste={(e) => void pasteImage(e)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  void send();
                }
              }}
              maxLength={100000}
            />
            {attached.length > 0 && (
              <div className="attachment-list" aria-label="Attached files">
                {attached.map((path) => {
                  const name = path.split(/[\\/]/).filter(Boolean).at(-1) || path;
                  return (
                    <span className="attachment-chip" key={path} title={path}>
                      <span>{name}</span>
                      <button
                        type="button"
                        aria-label={`Remove ${name}`}
                        onClick={() => setAttached((current) => current.filter((p) => p !== path))}
                      >
                        <X size={12} />
                      </button>
                    </span>
                  );
                })}
              </div>
            )}
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
              <ModeControl
                mode={mode}
                bothAvailable={accessMode === 'read-only'}
                onChange={(next) => {
                  setMode(next);
                  setAccountId('');
                  setModel('');
                  setEffort('');
                  setCatalogReady(false);
                  setAdvanced(false);
                }}
              />
              {mode === 'auto' && appSettings?.orchestra && (
                <button
                  className="orchestra-badge"
                  title={t(
                    'Orchestra is on: complex tasks are planned and split across agents. Click to turn off.',
                  )}
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
              {workspaces.length > 0 && (
                <div className="workspace-controls">
                  <select
                    aria-label="Project"
                    title={namedWorkspace?.name || t('Choose project')}
                    value={currentWorkspaceId}
                    disabled={running || folderBusy}
                    onChange={(e) => void chooseWorkspace(e.target.value)}
                  >
                    {!currentWorkspaceId && <option value="">{t('Choose project')}</option>}
                    {workspaces.map((w) => (
                      <option key={w.id} value={w.id}>
                        {w.name}
                      </option>
                    ))}
                  </select>
                </div>
              )}
              {(mode === 'claude' || mode === 'gpt') && (
                <button
                  className={`icon-button advanced-toggle ${advanced ? 'on' : ''}`}
                  aria-label={t('Advanced provider options')}
                  aria-expanded={advanced}
                  onClick={() => setAdvanced(!advanced)}
                >
                  {advanced ? <ChevronDown size={15} /> : <ChevronUp size={15} />}
                </button>
              )}
              <div className="composer-spacer" />
              <PermissionButton
                accessMode={accessMode}
                onCycle={(next) => void changeAccessMode(next)}
              />
              <button
                className={`icon-button attach-button ${attached.length ? 'on' : ''}`}
                aria-label={t('Attach files')}
                title={attached.length ? t(`${attached.length} files attached`) : t('Attach files')}
                onClick={() => void attachFiles()}
              >
                <Paperclip size={15} />
                {attached.length > 0 && <small>{attached.length}</small>}
              </button>
              <button
                className="send-button"
                aria-label={running ? t('Stop response') : t('Send message')}
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
                className={`token-badge ${blob ? 'warn' : impact.toLowerCase()}`}
                title={
                  false
                    ? 'Attachments are not included in this estimate'
                    : blob
                      ? 'This message contains a large encoded blob'
                      : `Estimated prompt size · quota impact ${impact}`
                }
              >
                ~{compactTokens(draftTokens)} tokens · {blob ? 'Heavy' : impact}
              </span>
            )}
            <span>
              {t('Enter to send')} <span className="caption-dot">·</span>{' '}
              {t('Shift Enter for a new line')}
            </span>
          </div>
        </div>
      </main>
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
      />
    </div>
  );
}
