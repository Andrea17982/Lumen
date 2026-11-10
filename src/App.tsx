import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import type { ClipboardEvent } from 'react';
import { useReducedMotion } from 'framer-motion';
import { Check, Folder, Plus, Search, Sparkles, X, Circle, Shrink } from 'lucide-react';
import ProviderOptions from './components/ProviderOptions';
import QuitConfirm from './components/QuitConfirm';
import { saveSettings, useAppSettings } from './lib/settings';
import type { SettingsTab } from './settings/SettingsPanel';
import { visibleInterval } from './lib/visible';
import { useCaptureHotkey } from './lib/useCaptureHotkey';
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
import { formatResetCountdown } from './lib/usage';
import { localAssistant } from './features/local-assistant/instance';
import { nameNewChat } from './features/local-assistant/autoTitle';
import { Draft } from './features/local-assistant/draft';
import DraftSheet from './components/DraftSheet';
import MessageView from './components/MessageView';
import { AttachmentList, ErrorBanner, PlanLine } from './components/ComposerParts';
import { ProjectList, SessionList } from './components/Sidebar';
import { active, laneOf, mergeUsage, providerOf } from './lib/messages';
import Welcome from './components/Welcome';
import Topbar, { type HealthPill } from './components/Topbar';
import { ComposerToolbar, TokenCaption } from './components/Composer';
import UsagePopover from './components/UsagePopover';
import ContextMeter from './components/ContextMeter';
import CommandPalette from './components/CommandPalette';
import { runCommand, type Command } from './lib/commands';
import { appCommands, conversationCommands } from './lib/appCommands';
import {
  contextWindow,
  estimateTokens,
  lastMeasurement,
  contextUse as measureContext,
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
} from './lib/types';
import { errorText, Mark, nameOf } from './lib/util';
import { useI18n } from './lib/i18n';

// Loaded on first use, so starting Lumen parses less code.
const SettingsPanel = lazy(() => import('./settings/SettingsPanel'));
const GatekeeperPanel = lazy(() => import('./features/gatekeeper/GatekeeperPanel'));
const ProjectInspector = lazy(() => import('./components/ProjectInspector'));
const ManualMerge = lazy(() => import('./components/ManualMerge'));
const tokens = estimateTokens;

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
  const [slashDismissed, setSlashDismissed] = useState(false);
  const [slashIndex, setSlashIndex] = useState(0);
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
  const [mode, setMode] = useState<Mode>(() => {
    const saved = localStorage.getItem('lumen-provider-mode');
    return ['claude', 'gpt', 'gemini', 'antigravity', 'ollama', 'auto', 'both'].includes(
      saved || '',
    )
      ? (saved as Mode)
      : 'claude';
  });
  const [accountId, setAccountId] = useState('');
  const [model, setModel] = useState('');
  const [effort, setEffort] = useState('');
  const [accessMode, setAccessMode] = useState<AccessMode>('read-only');
  const [catalogReady, setCatalogReady] = useState(false);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [workspaceId, setWorkspaceId] = useState(
    () => localStorage.getItem('lumen-workspace-id') || '',
  );
  const [folderBusy, setFolderBusy] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [settings, setSettings] = useState(false);
  const [inspector, setInspector] = useState(false);
  // The settings code loads the first time the panel opens, then stays mounted for its exit animation.
  const [settingsUsed, setSettingsUsed] = useState(false);
  if (settings && !settingsUsed) setSettingsUsed(true);
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
  /** Last background quota query per account; -1 after a failure. */
  const usageAttempts = useRef(new Map<string, number>());
  useEffect(() => {
    return visibleInterval(() => setUsageClock(Date.now()), 15_000);
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
  const slashMatch = /^\/([a-z]*)$/i.exec(draft);
  const slashCommands = [
    { name: 'workspace', description: t('Change the conversation folder') },
    { name: 'compact', description: t('Continue in a fresh session with a summary') },
  ].filter((command) => command.name.startsWith(slashMatch?.[1].toLowerCase() ?? ''));
  const showSlash = !!slashMatch && !slashDismissed && slashCommands.length > 0;
  // The backend sizes the prompt the next turn would send. It refreshes when a run ends, the
  // chat changes or its permission mode changes; until then the meter uses visible text.
  const [promptEstimate, setPromptEstimate] = useState<{ id: string; tokens: number } | null>(null);
  const [draftEstimate, setDraftEstimate] = useState<{ text: string; tokens: number } | null>(null);
  useEffect(() => {
    let active = true;
    const timer = setTimeout(() => {
      void call<number>('count_tokens', { text: draft })
        .then((count) => {
          if (active) setDraftEstimate({ text: draft, tokens: count });
        })
        .catch(() => {});
    }, 150);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [draft]);
  const sessionId = session?.id;
  const sessionAccess = session?.accessMode;
  useEffect(() => {
    if (!sessionId || running) return;
    let gone = false;
    void call<number>('prompt_estimate', { id: sessionId })
      .then((tokens) => {
        if (!gone) setPromptEstimate({ id: sessionId, tokens });
      })
      .catch(() => {
        if (!gone) setPromptEstimate(null);
      });
    return () => {
      gone = true;
    };
  }, [sessionId, sessionAccess, running]);
  const currentWorkspace = session?.workspace || workspace;
  const attachCapture = useCallback(
    (path: string) => setAttached((previous) => [...new Set([...previous, path])]),
    [],
  );
  useCaptureHotkey(currentWorkspace, attachCapture, setError);
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
    // The panel moves focus in itself once its code has loaded.
    settingsOpener.current = document.activeElement as HTMLElement | null;
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
    localStorage.setItem('lumen-provider-mode', mode);
  }, [mode]);
  useEffect(() => {
    if (
      accounts.length &&
      mode !== 'auto' &&
      mode !== 'both' &&
      !accounts.some((a) => a.provider === mode)
    ) {
      setMode(
        accounts.find((a) => ['claude', 'gpt'].includes(a.provider))?.provider ||
          accounts[0].provider,
      );
    }
  }, [accounts, mode]);
  const loadUsage = useCallback(async () => {
    const saved = await call<AccountUsage[]>('get_usage');
    setUsage((old) => mergeUsage(old, saved));
    await Promise.all(
      accounts
        .filter((a) => ['gpt', 'antigravity'].includes(a.provider))
        .filter((a) => {
          // Antigravity's quota query is a full CLI run, and an expired sign-in makes it open
          // the browser. Ask it rarely, and not again after a failure until Lumen restarts.
          if (a.provider !== 'antigravity') return true;
          const last = usageAttempts.current.get(a.id);
          return last !== -1 && (last === undefined || Date.now() - last > 15 * 60_000);
        })
        .map((a) => {
          const pending = usageRequests.current.get(a.id);
          if (pending) return pending;
          const request = (async () => {
            usageAttempts.current.set(a.id, Date.now());
            try {
              const row = await call<AccountUsage>('refresh_usage', { id: a.id });
              setUsage((old) => mergeUsage(old, [row]));
              setUsageErrors((old) => ({ ...old, [a.id]: '' }));
            } catch (error) {
              usageAttempts.current.set(a.id, -1);
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
    return visibleInterval(refreshUsage, 30_000);
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
    return visibleInterval(() => {
      load();
      setTick((n) => n + 1);
    }, 15_000);
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
    // Streamed tokens arrive many times per frame. Apply them once per animation frame, so the
    // transcript (and its markdown) renders at most once per frame instead of once per token.
    let queued: RunEvent[] = [];
    let frame = 0;
    const flush = () => {
      frame = 0;
      const batch = queued.filter((e) => e.sessionId === selectedRef.current);
      queued = [];
      if (batch.length) setMessages((m) => batch.reduce(applyEvent, m));
    };
    void subscribe((event: RunEvent) => {
      if (event.kind === 'workspace') {
        const updated = JSON.parse(event.text) as Session;
        if (activeSession.current?.id === updated.id) activeSession.current = updated;
        setSessions((list) => list.map((s) => (s.id === updated.id ? updated : s)));
        return;
      }
      queued.push(event);
      if (event.kind === 'delta' || event.kind === 'thinking' || event.kind === 'orchestra-delta') {
        if (!frame) frame = requestAnimationFrame(flush);
      } else {
        if (frame) cancelAnimationFrame(frame);
        flush();
      }
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
      if (frame) cancelAnimationFrame(frame);
      dispose();
    };
  }, []);
  useEffect(() => {
    // While a reply streams, follow it without animation: a smooth scroll restarted every frame
    // stutters and keeps the compositor busy.
    bottom.current?.scrollIntoView({
      behavior: reduced || running ? 'instant' : 'smooth',
      block: 'end',
    });
  }, [messages, reduced, running]);
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
    const manual = !override && mode !== 'auto' && mode !== 'both';
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
  async function send(
    override?: { text: string; mode: Mode; auditOf: string },
    commandText?: string,
  ) {
    const text = override?.text ?? commandText ?? draft.trim();
    const runMode = override?.mode ?? mode;
    if (!text || folderBusy) return;
    if (running) {
      if (!override && /^\/(compact|workspace)(?:\s|$)/i.test(text))
        setError(t('Wait for the current response to finish before using this command.'));
      return;
    }
    if (!override && text.toLowerCase() === '/workspace') {
      setError(t('Enter an absolute path after /workspace.'));
      return;
    }
    if (!override && text.toLowerCase() === '/compact') {
      if (!session || !messages.length) {
        setError(t('Open a conversation with messages before compacting.'));
        return;
      }
      setBusy(true);
      setError('');
      try {
        if (await compact()) setDraft('');
      } finally {
        setBusy(false);
        input.current?.focus();
      }
      return;
    }
    if (desktop && !override && mode !== 'auto' && mode !== 'both' && !catalogReady) {
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
    if (!session) return false;
    try {
      const next = await call<Session>('compact_session', { id: session.id, summary });
      await refresh();
      setSelected(next.id);
      setSheet(null);
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  }
  function selectSlash(name: string) {
    setSlashDismissed(true);
    setDraft(name === 'workspace' ? '/workspace ' : '/compact');
    input.current?.focus();
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
  const draftTokens =
    (draftEstimate?.text === draft ? draftEstimate.tokens : tokens(draft)) + attachedTokens;
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
    promptEstimate && promptEstimate.id === session?.id ? promptEstimate.tokens : historyTokens;
  // Only Claude reports its window and CLI overhead; other providers stay a local estimate.
  const contextUse = contextProvider
    ? measureContext(
        promptTokens,
        draftTokens,
        contextWindow(contextProvider, model || plan?.model || ''),
        ['claude', 'gpt'].includes(contextProvider) ? lastMeasurement(messages) : null,
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
  const messageView = (m: Message, inLane = false) => (
    <MessageView
      key={m.id}
      m={m}
      inLane={inLane}
      latest={m.id === messages.at(-1)?.id}
      live={running && m.role === 'assistant' && m.id === messages.at(-1)?.id}
      running={running}
      busy={busy}
      readOnly={accessMode === 'read-only'}
      workspace={session?.workspace}
      accounts={accounts}
      onRewind={session ? (x) => void askRewind(x) : undefined}
      onAudit={audit}
      onApplyLane={(x) => void resolveLanes(x, true)}
      onOrchestra={orchestra}
      onError={setError}
    />
  );
  const newChat = () => {
    setSelected(null);
    setMessages([]);
    setDraft('');
    setSidebar(false);
    input.current?.focus();
  };

  /** Actions on one conversation; the sidebar menu and the palette run the same entries. */
  const sessionCommands = (s: Session, at?: { x: number; y: number }) =>
    conversationCommands(s, at, {
      rename: (target, pos) => {
        setRenameTarget(target);
        setRenameValue(target.title);
        setRenamePos(pos);
      },
      pin: (target) => void update(target, target.title, !target.pinned),
      remove: setDeleteTarget,
    });
  const commands = appCommands({
    session,
    canAct: !!session && !running,
    hasMessages: messages.length > 0,
    newChat,
    newProject: () => {
      setSidebar(true);
      setProjectForm(true);
    },
    search: () => {
      setSidebar(true);
      requestAnimationFrame(() => searchInput.current?.focus());
    },
    compact: () => void compact(),
    handoff: openHandoff,
    commit: () => void openCommit(),
    usage: () => setUsageOpen(true),
    settings: openSettings,
    inspector: currentWorkspace ? () => setInspector((v) => !v) : undefined,
    access: (mode) => void changeAccessMode(mode),
    theme: setTheme,
    sessionCommands,
  });
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
        <ProjectList
          workspaces={workspaces}
          filter={projectFilter}
          onFilter={(id) => {
            setProjectFilter(id);
            if (id !== '*') {
              setWorkspaceId(id);
              localStorage.setItem('lumen-workspace-id', id);
            }
          }}
          onDropSession={(id, workspace) => {
            const s = sessions.find((s) => s.id === id);
            if (s) void moveSession(s, workspace);
          }}
        />
        <div className="session-heading">
          <span>{t('Your conversations')}</span>
          <span>
            {sessions.length}
            {hasMore ? '+' : ''}
          </span>
        </div>
        <SessionList
          sessions={sessions.filter(
            (s) => projectFilter === '*' || (s.workspaceId || '') === projectFilter,
          )}
          empty={sessions.length === 0}
          searching={!!search}
          selected={selected}
          menuOpenFor={context?.session.id}
          hasMore={hasMore}
          onSelect={(id) => {
            setSelected(id);
            setSidebar(false);
          }}
          onReorder={(id, beforeId) =>
            void call('reorder_session', { id, beforeId })
              .then(refresh)
              .catch((err) => setError(errorText(err)))
          }
          onMenu={(session, x, y, trigger) => {
            contextTrigger.current = trigger;
            setContext({ session, x, y });
          }}
          onLoadMore={() => void loadMore()}
        />
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
          onOpenInspector={currentWorkspace ? () => setInspector((v) => !v) : undefined}
        />
        {inspector && currentWorkspace && (
          <Suspense fallback={null}>
            <ProjectInspector
              folder={currentWorkspace}
              session={session?.id}
              onClose={() => setInspector(false)}
              onError={setError}
            />
          </Suspense>
        )}
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
                    <div className="lane-grid">{g.map((m) => messageView(m, true))}</div>
                    {session &&
                      !running &&
                      g.some((m) => m.activity.some((a) => a.kind === 'worktree')) && (
                        <Suspense fallback={null}>
                          <ManualMerge
                            session={session.id}
                            message={
                              g.find((m) => m.activity.some((a) => a.kind === 'worktree'))!.id
                            }
                            onApplied={async () => {
                              setMessages(
                                await call<Message[]>('get_messages', { id: session.id }),
                              );
                              await refresh();
                            }}
                          />
                        </Suspense>
                      )}
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
                  messageView(g)
                ),
              )}
              <div ref={bottom} />
            </div>
          )}
        </div>

        <div className="composer-region">
          {error && (
            <ErrorBanner text={error} label={t('Dismiss error')} onDismiss={() => setError('')} />
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
              <Suspense fallback={null}>
                <GatekeeperPanel
                  requests={approvals.requests.filter(
                    (r) => r.status === 'pending' || r.delivery === 'uncertain',
                  )}
                  sessions={approvals.sessions}
                  onDecide={decideApproval}
                />
              </Suspense>
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
          {plan && <PlanLine plan={plan} />}
          <div className="composer">
            {showSlash && (
              <div
                className="slash-menu"
                id="slash-menu"
                role="listbox"
                aria-label={t('Available commands')}
              >
                {slashCommands.map((command, index) => (
                  <button
                    key={command.name}
                    id={`slash-${command.name}`}
                    type="button"
                    role="option"
                    aria-selected={index === Math.min(slashIndex, slashCommands.length - 1)}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => selectSlash(command.name)}
                  >
                    <strong>/{command.name}</strong>
                    <span>{command.description}</span>
                  </button>
                ))}
              </div>
            )}
            <textarea
              ref={input}
              aria-label="Message"
              aria-autocomplete="list"
              aria-controls={showSlash ? 'slash-menu' : undefined}
              aria-activedescendant={
                showSlash
                  ? `slash-${slashCommands[Math.min(slashIndex, slashCommands.length - 1)].name}`
                  : undefined
              }
              placeholder={t("What's on your mind?")}
              value={draft}
              onChange={(e) => {
                setDraft(e.target.value);
                setSlashDismissed(false);
                setSlashIndex(0);
              }}
              onPaste={(e) => void pasteImage(e)}
              onKeyDown={(e) => {
                if (showSlash && !e.nativeEvent.isComposing) {
                  if (e.key === 'Escape') {
                    e.preventDefault();
                    e.stopPropagation();
                    setSlashDismissed(true);
                    return;
                  }
                  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
                    e.preventDefault();
                    setSlashIndex(
                      (current) =>
                        (current + (e.key === 'ArrowDown' ? 1 : -1) + slashCommands.length) %
                        slashCommands.length,
                    );
                    return;
                  }
                  if ((e.key === 'Enter' && !e.shiftKey) || e.key === 'Tab') {
                    e.preventDefault();
                    selectSlash(slashCommands[Math.min(slashIndex, slashCommands.length - 1)].name);
                    return;
                  }
                }
                if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  void send();
                }
              }}
              maxLength={100000}
            />
            <AttachmentList
              paths={attached}
              onRemove={(path) => setAttached((current) => current.filter((p) => p !== path))}
            />
            {mode !== 'auto' && mode !== 'both' && (
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
            <ComposerToolbar
              mode={mode}
              bothAvailable={accessMode === 'read-only'}
              providers={[...new Set(accounts.map((a) => a.provider))]}
              onMode={(next) => {
                setMode(next);
                setAccountId('');
                setModel('');
                setEffort('');
                setCatalogReady(false);
                setAdvanced(false);
              }}
              orchestra={mode !== 'both' && !!appSettings?.orchestra}
              onOrchestraOff={() =>
                appSettings &&
                void saveSettings({ ...appSettings, orchestra: false }).catch((e) =>
                  setError(errorText(e)),
                )
              }
              workspaces={workspaces}
              workspaceId={currentWorkspaceId}
              workspaceName={namedWorkspace?.name}
              workspaceLocked={running || folderBusy}
              onWorkspace={(id) => void chooseWorkspace(id)}
              advanced={advanced}
              onAdvanced={() => setAdvanced(!advanced)}
              accessMode={accessMode}
              onAccess={(next) => void changeAccessMode(next)}
              attached={attached.length}
              onAttach={() => void attachFiles()}
              running={running}
              canSend={
                !!draft.trim() &&
                !folderBusy &&
                (draft.trim().toLowerCase() === '/compact' ||
                  !(desktop && mode !== 'auto' && mode !== 'both' && !catalogReady))
              }
              onSend={() => {
                if (running && selected)
                  void call('cancel_run', { id: selected }).catch((e) => setError(errorText(e)));
                else void send();
              }}
            />
          </div>
          <TokenCaption tokens={draft.trim() ? draftTokens : 0} impact={impact} heavy={blob} />
        </div>
      </main>
      <QuitConfirm />
      {settingsUsed && (
        <Suspense fallback={null}>
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
        </Suspense>
      )}
    </div>
  );
}
