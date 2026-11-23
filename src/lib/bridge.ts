import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { recordActivity } from './activity';
import type {
  Account,
  AccountUsage,
  AppSettings,
  Message,
  RunEvent,
  Session,
  Workspace,
  TaskWorktreeStatus,
} from './types';
import type { ApprovalRequest, Decision, SessionView } from '../features/gatekeeper/contracts';

export function onUsage(handler: (usage: AccountUsage) => void) {
  return desktop
    ? listen<AccountUsage>('lumen:usage', (event) => handler(event.payload))
    : Promise.resolve(() => {});
}

export const desktop = isTauri();
const recentEvents: RunEvent[] = [];
function remember(event: RunEvent) {
  if (!event.seq) return;
  recentEvents.push(event);
  if (recentEvents.length > 10000) recentEvents.splice(0, recentEvents.length - 10000);
}
/** A snapshot can finish before an already received event reaches React's state queue. */
export function mergeMessages(snapshot: Message[], current: Message[]): Message[] {
  const watermark = snapshot[0]?.snapshotSeq ?? 0;
  const replayed = recentEvents
    .filter(
      (e) =>
        (e.seq || 0) > watermark && [...snapshot, ...current].some((m) => m.id === e.messageId),
    )
    .reduce(applyEvent, snapshot);
  const next = replayed.map((m) => {
    const newer = current.find((c) => c.id === m.id && (c.eventSeq || 0) > (m.eventSeq || 0));
    return newer || m;
  });
  const seq = Math.max(0, ...snapshot.map((m) => m.snapshotSeq || m.eventSeq || 0));
  for (const m of current)
    if ((m.eventSeq || 0) > seq && !next.some((n) => n.id === m.id)) next.push(m);
  return next;
}
const key = 'lumen-preview-v1';
function read(): {
  sessions: Session[];
  accounts: Account[];
  workspaces?: Workspace[];
  messages?: Record<string, Message[]>;
  /** Task worktrees by session, for the preview of Publish, Apply and Discard. */
  tasks?: Record<string, TaskWorktreeStatus>;
  usage?: AccountUsage[];
} {
  try {
    return JSON.parse(localStorage.getItem(key) || '{"sessions":[],"accounts":[]}');
  } catch {
    return { sessions: [], accounts: [] };
  }
}
const settingsKey = 'lumen-preview-settings';
function previewSettings(): AppSettings {
  const base: AppSettings = {
    verifyChanges: false,
    captureHotkey: false,
    autoFallback: true,
    selfHeal: true,
    reviewRounds: 1,
    orchestra: false,
    closeToTray: true,
    openAtLogin: false,
    ollamaModel: '',
    orchestraLimits: {
      maxConcurrent: null,
      maxSubtasks: null,
      maxMinutes: null,
      quotaThresholds: {},
    },
    repoContext: true,
    codeMode: 'auto',
  };
  try {
    return { ...base, ...JSON.parse(localStorage.getItem(settingsKey) || '{}') };
  } catch {
    return base;
  }
}
/** Browser preview never launches a CLI or pretends to generate an AI response. */
export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (desktop) {
    const value = await invoke<T>(command, args);
    if (command === 'get_messages') {
      const watermark = (value as Message[])[0]?.snapshotSeq ?? Number.POSITIVE_INFINITY;
      return recentEvents
        .filter((e) => e.sessionId === args.id && (e.seq || 0) > watermark)
        .reduce(applyEvent, value as Message[]) as T;
    }
    return value;
  }
  const data = read();
  let result: unknown;
  switch (command) {
    case 'list_workspaces':
      result = data.workspaces || [];
      break;
    case 'add_workspace': {
      const workspace: Workspace = {
        id: crypto.randomUUID(),
        name: String(args.name),
        folder: String(args.folder),
      };
      data.workspaces = [...(data.workspaces || []), workspace];
      result = workspace;
      break;
    }
    case 'rename_workspace':
      data.workspaces = data.workspaces?.map((w) =>
        w.id === args.id ? { ...w, name: String(args.name) } : w,
      );
      break;
    case 'delete_workspace': {
      const removed = data.sessions.filter((s) => s.workspaceId === args.id).map((s) => s.id);
      data.sessions = data.sessions.filter((s) => s.workspaceId !== args.id);
      for (const id of removed) if (data.messages) delete data.messages[id];
      data.workspaces = data.workspaces?.filter((w) => w.id !== args.id);
      break;
    }
    case 'set_session_workspace':
      data.sessions = data.sessions.map((s) =>
        s.id === args.id
          ? { ...s, workspace: String(args.folder), workspaceId: args.workspaceId as string | null }
          : s,
      );
      result = data.sessions.find((s) => s.id === args.id);
      break;
    case 'rename_account':
      data.accounts = data.accounts.map((a) =>
        a.id === args.id ? { ...a, name: String(args.name) } : a,
      );
      break;
    case 'list_sessions':
      result = data.sessions
        .filter((s) => s.title.toLowerCase().includes(String(args.search || '').toLowerCase()))
        .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt)
        .slice(Number(args.offset), Number(args.offset) + 100);
      break;
    case 'create_session': {
      const s: Session = {
        id: crypto.randomUUID(),
        title: 'New conversation',
        pinned: false,
        status: 'idle',
        workspace: String(args.workspace),
        updatedAt: Date.now(),
      };
      data.sessions.unshift(s);
      result = s;
      break;
    }
    case 'update_session':
      data.sessions = data.sessions.map((s) =>
        s.id === args.id ? { ...s, title: String(args.title), pinned: Boolean(args.pinned) } : s,
      );
      break;
    case 'delete_session':
      data.sessions = data.sessions.filter((s) => s.id !== args.id);
      break;
    case 'get_messages':
      result = data.messages?.[String(args.id)] || [];
      break;
    case 'activity_detail':
      // Browser preview keeps full text beside a bounded fixture, keyed like the daemon's digest.
      result = (data as { details?: Record<string, string> }).details?.[String(args.digest)];
      if (result === undefined) throw new Error('This output is no longer available');
      break;
    case 'list_accounts':
      result = data.accounts;
      break;
    case 'get_usage':
      result = data.usage || [];
      break;
    case 'get_settings':
      result = previewSettings();
      break;
    case 'save_settings':
      localStorage.setItem(settingsKey, JSON.stringify(args.value));
      result = previewSettings();
      break;
    case 'provider_health':
    case 'mcp_health':
    case 'project_tools':
    case 'project_checks':
    case 'change_layers':
    case 'plugins_list':
      result = [];
      break;
    case 'repo_map_search':
      result = {
        symbols: [],
        stats: { files: 0, symbols: 0, refreshed: 0, removed: 0, millis: 0, partial: false },
      };
      break;
    case 'plugin_contributions':
      result = { themes: [], slashCommands: [], panels: [] };
      break;
    case 'ollama_status':
      result = { installed: false, running: false, models: [], selected: null };
      break;
    case 'project_context':
      result = { notes: '', files: [], verify: null, git: false };
      break;
    case 'diagnostics':
      result = { claude: false, gpt: false, dataDir: 'Desktop app required' };
      break;
    case 'task_worktree_status': {
      const task = data.tasks?.[String(args.id)];
      result = task ? { ...task, blocked: args.preview ? task.blocked : null } : null;
      break;
    }
    case 'publish_task': {
      const task = data.tasks?.[String(args.id)];
      if (!task) throw new Error('This conversation has no task worktree');
      const url = 'https://github.com/example/project/pull/1';
      task.pr = { url, watching: true, rounds: 0, last: 'Waiting for checks' };
      result = {
        session: data.sessions.find((s) => s.id === args.id),
        url,
      };
      break;
    }
    case 'stop_watching_task': {
      const pr = data.tasks?.[String(args.id)]?.pr;
      if (pr) Object.assign(pr, { watching: false, last: 'Not watched' });
      break;
    }
    case 'apply_task':
    case 'discard_task': {
      const task = data.tasks?.[String(args.id)];
      if (!task) throw new Error('This conversation has no task worktree');
      delete data.tasks![String(args.id)];
      data.sessions = data.sessions.map((s) =>
        s.id === args.id ? { ...s, workspace: task.origin } : s,
      );
      const session = data.sessions.find((s) => s.id === args.id);
      result = session;
      break;
    }
    default:
      throw new Error(
        'Open the Lumen desktop app to connect accounts and run AI sessions. This browser is a UI preview.',
      );
  }
  localStorage.setItem(key, JSON.stringify(data));
  return result as T;
}
export function subscribe(handler: (event: RunEvent) => void) {
  return desktop
    ? listen<RunEvent>('lumen:run', (e) => {
        remember(e.payload);
        handler(e.payload);
      })
    : previewEvents(handler);
}
/** Browser preview has no daemon; offline tests stream fixture events through this DOM event. */
function previewEvents(handler: (event: RunEvent) => void) {
  const listener = (e: Event) => handler((e as CustomEvent<RunEvent>).detail);
  window.addEventListener('lumen-preview-run', listener);
  return Promise.resolve(() => window.removeEventListener('lumen-preview-run', listener));
}
export async function onServiceChange(
  handler: (state: 'resync' | 'disconnected' | 'error', message?: string) => void,
) {
  if (!desktop) return () => {};
  const off = await Promise.all([
    listen('lumen:resync', () => handler('resync')),
    listen('lumen:daemon-disconnected', () => handler('disconnected')),
    listen<string>('lumen:daemon-error', (event) => handler('error', event.payload)),
  ]);
  return () => off.forEach((fn) => fn());
}
export function applyEvent(messages: Message[], event: RunEvent): Message[] {
  if (!event.messageId) return messages;
  const current = messages.find((m) => m.id === event.messageId);
  if (event.seq && (current?.eventSeq || 0) >= event.seq) return messages;
  return applyEventContent(messages, event).map((m) =>
    m.id === event.messageId && event.seq ? { ...m, eventSeq: event.seq } : m,
  );
}
function applyEventContent(messages: Message[], event: RunEvent): Message[] {
  const found = messages.some((m) => m.id === event.messageId);
  const next = found
    ? messages
    : [...messages, { id: event.messageId, role: 'assistant' as const, text: '', activity: [] }];
  return next.map((m) => {
    if (m.id !== event.messageId) return m;
    if (event.kind === 'delta') return { ...m, text: m.text + event.text };
    if (event.kind === 'replace') return { ...m, text: event.text };
    if (event.kind === 'orchestra') {
      const rest = m.activity.filter((a) => a.kind !== 'orchestra');
      return { ...m, activity: [{ kind: 'orchestra', text: event.text }, ...rest] };
    }
    if (event.kind === 'orchestra-delta') {
      try {
        const d = JSON.parse(event.text) as { id: string; kind: string; text: string };
        const prev = m.live?.[d.id] || '';
        const next = d.kind === 'replace' ? d.text : prev + d.text;
        return { ...m, live: { ...m.live, [d.id]: next.slice(-20000) } };
      } catch {
        return m;
      }
    }
    if (['start', 'done', 'status'].includes(event.kind)) return m;
    const last = m.activity.at(-1);
    // Reasoning streams in fragments; keep them as one block instead of one entry per token.
    if (event.kind === 'thinking' && last?.kind === 'thinking') {
      return {
        ...m,
        activity: [...m.activity.slice(0, -1), { kind: 'thinking', text: last.text + event.text }],
      };
    }
    const incoming = { kind: event.kind, text: event.text, detail: event.detail };
    if (!incoming.detail) delete incoming.detail;
    return { ...m, activity: recordActivity(m.activity, incoming) };
  });
}
export async function external(url: string) {
  if (desktop) {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  } else window.open(url, '_blank', 'noopener,noreferrer');
}

export interface ApprovalSnapshot {
  requests: ApprovalRequest[];
  sessions: SessionView[];
}
/** Approvals and live provider sessions for one chat. Empty outside the desktop app. */
export async function approvalSnapshot(chat: string): Promise<ApprovalSnapshot> {
  if (!desktop) return { requests: [], sessions: [] };
  return invoke<ApprovalSnapshot>('gatekeeper_snapshot', { chat });
}
/** Sends only the request ID, scope and action; the native broker holds the request itself. */
export async function decideApproval(decision: Decision): Promise<void> {
  if (!desktop) throw new Error('Approvals need the Lumen desktop app.');
  await invoke('gatekeeper_decide', { decision });
}
/** Fires on every approval change, and when the listener lagged and the snapshot must reload. */
export async function onApprovalChange(handler: () => void) {
  if (!desktop) return () => {};
  const off = await Promise.all([
    listen('lumen:gatekeeper', handler),
    listen('lumen:resync', handler),
  ]);
  return () => off.forEach((fn) => fn());
}
