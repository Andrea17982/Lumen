import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  Account,
  AccountUsage,
  AppSettings,
  Message,
  RunEvent,
  Session,
  Workspace,
} from './types';
import type { ApprovalRequest, Decision, SessionView } from '../features/gatekeeper/contracts';

export function onUsage(handler: (usage: AccountUsage) => void) {
  return desktop
    ? listen<AccountUsage>('lumen:usage', (event) => handler(event.payload))
    : Promise.resolve(() => {});
}

export const desktop = isTauri();
const key = 'lumen-preview-v1';
function read(): {
  sessions: Session[];
  accounts: Account[];
  workspaces?: Workspace[];
  messages?: Record<string, Message[]>;
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
    autoFallback: true,
    selfHeal: true,
    reviewRounds: 1,
    apiEnabled: false,
    apiPort: 8317,
    apiToken: 'preview-key-preview-key',
    orchestra: false,
    closeToTray: false,
    openAtLogin: false,
    ollamaModel: '',
    orchestraLimits: { maxConcurrent: null, maxSubtasks: null, quotaThresholds: {} },
    localAssistant: { enabled: false, model: '', autoTitles: true, timeoutMs: 60_000 },
  };
  try {
    return { ...base, ...JSON.parse(localStorage.getItem(settingsKey) || '{}') };
  } catch {
    return base;
  }
}
/** Browser preview never launches a CLI or pretends to generate an AI response. */
export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (desktop) return invoke<T>(command, args);
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
      result = [];
      break;
    case 'ollama_status':
      result = { installed: false, running: false, models: [], selected: null };
      break;
    case 'ollama_models':
      result = [];
      break;
    case 'api_status':
      result = { running: false, url: 'http://localhost:8317/v1', log: [] };
      break;
    case 'project_context':
      result = { notes: '', files: [], verify: null, git: false };
      break;
    case 'diagnostics':
      result = { claude: false, gpt: false, dataDir: 'Desktop app required' };
      break;
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
    ? listen<RunEvent>('lumen:run', (e) => handler(e.payload))
    : Promise.resolve(() => {});
}
export function applyEvent(messages: Message[], event: RunEvent): Message[] {
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
    return { ...m, activity: [...m.activity, { kind: event.kind, text: event.text }].slice(-500) };
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
export function onApprovalChange(handler: () => void) {
  return desktop ? listen('lumen:gatekeeper', () => handler()) : Promise.resolve(() => {});
}
